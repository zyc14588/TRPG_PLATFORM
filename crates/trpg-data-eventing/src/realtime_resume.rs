use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use zeroize::Zeroizing;

type HmacSha256 = Hmac<Sha256>;

const TOKEN_VERSION: &str = "rt1";
const MAX_TOKEN_BYTES: usize = 2_048;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RealtimeResumeClaims {
    pub key_id: String,
    pub tenant_id: String,
    pub campaign_id: String,
    pub user_id: String,
    pub authority_epoch: u64,
    pub cursor: u64,
    pub issued_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RealtimeResumeBinding<'a> {
    pub tenant_id: &'a str,
    pub campaign_id: &'a str,
    pub user_id: &'a str,
    pub authority_epoch: u64,
}

pub struct RealtimeResumeTokenCodec {
    key_id: String,
    key: Zeroizing<[u8; 32]>,
}

impl std::fmt::Debug for RealtimeResumeTokenCodec {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RealtimeResumeTokenCodec")
            .field("key_id", &self.key_id)
            .field("key", &"[REDACTED]")
            .finish()
    }
}

impl RealtimeResumeTokenCodec {
    pub fn new(key_id: impl Into<String>, key: &[u8]) -> Result<Self, RealtimeResumeError> {
        let key_id = key_id.into();
        validate_identifier(&key_id)?;
        if key.len() != 32 {
            return Err(RealtimeResumeError::InvalidConfiguration);
        }
        let mut protected = Zeroizing::new([0_u8; 32]);
        protected.copy_from_slice(key);
        Ok(Self {
            key_id,
            key: protected,
        })
    }

    /// Derives a purpose-separated resume key from an existing mounted
    /// 32-byte service secret without persisting or logging the derived key.
    pub fn derive(
        key_id: impl Into<String>,
        parent_key: &[u8],
    ) -> Result<Self, RealtimeResumeError> {
        if parent_key.len() != 32 {
            return Err(RealtimeResumeError::InvalidConfiguration);
        }
        let mut mac = HmacSha256::new_from_slice(parent_key)
            .map_err(|_| RealtimeResumeError::InvalidConfiguration)?;
        mac.update(b"trpg-realtime-resume-token-v1");
        let derived: [u8; 32] = mac.finalize().into_bytes().into();
        Self::new(key_id, &derived)
    }

    pub fn issue(
        &self,
        binding: &RealtimeResumeBinding<'_>,
        cursor: u64,
        issued_at_unix_ms: u64,
        expires_at_unix_ms: u64,
    ) -> Result<String, RealtimeResumeError> {
        validate_binding(binding)?;
        if issued_at_unix_ms == 0 || expires_at_unix_ms <= issued_at_unix_ms {
            return Err(RealtimeResumeError::InvalidClaims);
        }
        let claims = RealtimeResumeClaims {
            key_id: self.key_id.clone(),
            tenant_id: binding.tenant_id.to_owned(),
            campaign_id: binding.campaign_id.to_owned(),
            user_id: binding.user_id.to_owned(),
            authority_epoch: binding.authority_epoch,
            cursor,
            issued_at_unix_ms,
            expires_at_unix_ms,
        };
        let payload =
            serde_json::to_vec(&claims).map_err(|_| RealtimeResumeError::InvalidClaims)?;
        let payload = URL_SAFE_NO_PAD.encode(payload);
        let signature = self.sign(payload.as_bytes())?;
        Ok(format!(
            "{TOKEN_VERSION}.{payload}.{}",
            URL_SAFE_NO_PAD.encode(signature)
        ))
    }

    pub fn verify(
        &self,
        token: &str,
        binding: &RealtimeResumeBinding<'_>,
        expected_cursor: u64,
        now_unix_ms: u64,
    ) -> Result<RealtimeResumeClaims, RealtimeResumeError> {
        validate_binding(binding)?;
        if token.is_empty() || token.len() > MAX_TOKEN_BYTES {
            return Err(RealtimeResumeError::InvalidToken);
        }
        let mut fields = token.split('.');
        let version = fields.next();
        let payload = fields.next();
        let signature = fields.next();
        if version != Some(TOKEN_VERSION)
            || payload.is_none()
            || signature.is_none()
            || fields.next().is_some()
        {
            return Err(RealtimeResumeError::InvalidToken);
        }
        let payload = payload.expect("checked payload");
        let signature = URL_SAFE_NO_PAD
            .decode(signature.expect("checked signature"))
            .map_err(|_| RealtimeResumeError::InvalidToken)?;
        let mut mac = HmacSha256::new_from_slice(self.key.as_slice())
            .map_err(|_| RealtimeResumeError::InvalidConfiguration)?;
        mac.update(TOKEN_VERSION.as_bytes());
        mac.update(&[0]);
        mac.update(payload.as_bytes());
        mac.verify_slice(&signature)
            .map_err(|_| RealtimeResumeError::InvalidToken)?;
        let claims: RealtimeResumeClaims = serde_json::from_slice(
            &URL_SAFE_NO_PAD
                .decode(payload)
                .map_err(|_| RealtimeResumeError::InvalidToken)?,
        )
        .map_err(|_| RealtimeResumeError::InvalidToken)?;
        if claims.key_id != self.key_id
            || claims.tenant_id != binding.tenant_id
            || claims.campaign_id != binding.campaign_id
            || claims.user_id != binding.user_id
            || claims.authority_epoch != binding.authority_epoch
            || claims.cursor != expected_cursor
            || claims.issued_at_unix_ms == 0
            || claims.issued_at_unix_ms > now_unix_ms
            || claims.expires_at_unix_ms <= now_unix_ms
        {
            return Err(RealtimeResumeError::BindingMismatch);
        }
        Ok(claims)
    }

    fn sign(&self, payload: &[u8]) -> Result<[u8; 32], RealtimeResumeError> {
        let mut mac = HmacSha256::new_from_slice(self.key.as_slice())
            .map_err(|_| RealtimeResumeError::InvalidConfiguration)?;
        mac.update(TOKEN_VERSION.as_bytes());
        mac.update(&[0]);
        mac.update(payload);
        Ok(mac.finalize().into_bytes().into())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RealtimeResumeError {
    InvalidConfiguration,
    InvalidClaims,
    InvalidToken,
    BindingMismatch,
}

impl RealtimeResumeError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidConfiguration => "REALTIME_RESUME_CONFIGURATION_INVALID",
            Self::InvalidClaims => "REALTIME_RESUME_CLAIMS_INVALID",
            Self::InvalidToken => "REALTIME_RESUME_TOKEN_INVALID",
            Self::BindingMismatch => "REALTIME_RESUME_BINDING_MISMATCH",
        }
    }
}

impl std::fmt::Display for RealtimeResumeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for RealtimeResumeError {}

fn validate_binding(binding: &RealtimeResumeBinding<'_>) -> Result<(), RealtimeResumeError> {
    for value in [binding.tenant_id, binding.campaign_id, binding.user_id] {
        validate_identifier(value)?;
    }
    if binding.authority_epoch == 0 {
        return Err(RealtimeResumeError::InvalidClaims);
    }
    Ok(())
}

fn validate_identifier(value: &str) -> Result<(), RealtimeResumeError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        return Err(RealtimeResumeError::InvalidClaims);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: [u8; 32] = [0x51; 32];

    fn binding<'a>(user_id: &'a str, authority_epoch: u64) -> RealtimeResumeBinding<'a> {
        RealtimeResumeBinding {
            tenant_id: "tenant_resume",
            campaign_id: "campaign_resume",
            user_id,
            authority_epoch,
        }
    }

    #[test]
    fn resume_token_is_bound_to_user_campaign_epoch_cursor_and_expiry() {
        let codec =
            RealtimeResumeTokenCodec::derive("resume_key_v1", &KEY).expect("valid derived codec");
        let token = codec
            .issue(&binding("player_a", 7), 41, 1_000, 5_000)
            .expect("issue resume token");
        let claims = codec
            .verify(&token, &binding("player_a", 7), 41, 2_000)
            .expect("verify exact resume binding");
        assert_eq!(claims.cursor, 41);

        assert_eq!(
            codec
                .verify(&token, &binding("player_b", 7), 41, 2_000)
                .unwrap_err(),
            RealtimeResumeError::BindingMismatch
        );
        assert_eq!(
            codec
                .verify(&token, &binding("player_a", 8), 41, 2_000)
                .unwrap_err(),
            RealtimeResumeError::BindingMismatch
        );
        assert_eq!(
            codec
                .verify(&token, &binding("player_a", 7), 40, 2_000)
                .unwrap_err(),
            RealtimeResumeError::BindingMismatch
        );
        assert_eq!(
            codec
                .verify(&token, &binding("player_a", 7), 41, 5_000)
                .unwrap_err(),
            RealtimeResumeError::BindingMismatch
        );
    }

    #[test]
    fn resume_token_tampering_and_key_rotation_fail_closed() {
        let codec =
            RealtimeResumeTokenCodec::derive("resume_key_v1", &KEY).expect("valid derived codec");
        let token = codec
            .issue(&binding("player_a", 1), 3, 1_000, 5_000)
            .expect("issue resume token");
        let mut tampered = token.clone().into_bytes();
        let last = tampered.last_mut().expect("token byte");
        *last = if *last == b'A' { b'B' } else { b'A' };
        let tampered = String::from_utf8(tampered).expect("ascii token");
        assert_eq!(
            codec
                .verify(&tampered, &binding("player_a", 1), 3, 2_000)
                .unwrap_err(),
            RealtimeResumeError::InvalidToken
        );

        let rotated = RealtimeResumeTokenCodec::derive("resume_key_v1", &[0x52; 32])
            .expect("rotated derived codec");
        assert_eq!(
            rotated
                .verify(&token, &binding("player_a", 1), 3, 2_000)
                .unwrap_err(),
            RealtimeResumeError::InvalidToken
        );
    }
}
