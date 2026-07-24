crate::define_data_event_module!(
    CacheRedisImplCommand,
    CacheRedisImplOperation,
    append_cache_redis_impl_event,
    "cache_redis_impl",
    "CacheRedisImplRecorded",
    "data_eventing.cache_redis_impl.event_schema",
    crate::DataEventOperation::CacheWrite,
    ["event_store", "redis_cache", "projection_checkpoint"]
);

crate::define_data_event_artifacts!(
    CacheRedisImplService,
    CacheRedisImplRepository,
    CacheRedisImplEvent,
    CacheRedisImplError,
    EVENT_TYPE,
    EVENT_SCHEMA_NAME
);

pub const CACHE_REBUILD_SOURCE: &str = crate::EVENT_STORE_TABLE;
pub const CACHE_IS_CANONICAL: bool = false;

use std::fmt;
use std::sync::Arc;

use hmac::{Hmac, Mac};
use redis::aio::ConnectionManager;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use trpg_identity::ReplayAuthorization;
use trpg_privacy::PayloadCipher;
use trpg_shared_kernel::{EntityId, Visibility};
use url::Url;
use zeroize::{Zeroize, Zeroizing};

type HmacSha256 = Hmac<Sha256>;

const CACHE_SCHEMA_VERSION: u16 = 1;
const MAX_CACHE_VALUE_BYTES: usize = 1_048_576;

const MONOTONIC_PROJECTION_SCRIPT: &str = r#"
local function extend_ttl(key, proposed)
    local current_ttl = redis.call('TTL', key)
    local proposed_ttl = tonumber(proposed)
    if current_ttl < proposed_ttl then
        redis.call('EXPIRE', key, proposed_ttl)
    end
end

local current = redis.call('GET', KEYS[1])
if current then
    local decoded = cjson.decode(current)
    local current_version = tonumber(decoded['version'])
    local proposed_version = tonumber(ARGV[1])
    if current_version > proposed_version then
        return -1
    end
    if current_version == proposed_version then
        if decoded['entry_digest'] ~= ARGV[4] then
            return -2
        end
        extend_ttl(KEYS[1], ARGV[3])
        redis.call('SADD', KEYS[2], KEYS[1])
        extend_ttl(KEYS[2], ARGV[3])
        return 1
    end
    if decoded['subject_index_key'] then
        redis.call('SREM', decoded['subject_index_key'], KEYS[1])
    end
end
redis.call('SET', KEYS[1], ARGV[2], 'EX', ARGV[3])
redis.call('SADD', KEYS[2], KEYS[1])
extend_ttl(KEYS[2], ARGV[3])
return 1
"#;

const INVALIDATE_ENTRY_SCRIPT: &str = r#"
local current = redis.call('GET', KEYS[1])
if not current then
    return 0
end
local decoded = cjson.decode(current)
if decoded['subject_index_key'] then
    redis.call('SREM', decoded['subject_index_key'], KEYS[1])
end
return redis.call('DEL', KEYS[1])
"#;

const INVALIDATE_SUBJECT_SCRIPT: &str = r#"
local members = redis.call('SMEMBERS', KEYS[1])
local removed = 0
for _, key in ipairs(members) do
    removed = removed + redis.call('DEL', key)
end
redis.call('DEL', KEYS[1])
return removed
"#;

/// A decrypted cache value. Fields are private and Debug redacts the logical
/// key, provenance reference, data subject, and JSON value.
#[derive(Clone, PartialEq, Eq)]
pub struct ProjectionCacheEntry {
    key: String,
    campaign_id: String,
    data_subject_id: String,
    version: i64,
    visibility_label: String,
    visibility_subject: String,
    provenance_kind: String,
    provenance_reference: String,
    value_json: String,
    ttl_seconds: u64,
}

impl fmt::Debug for ProjectionCacheEntry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProjectionCacheEntry")
            .field("key", &"[REDACTED]")
            .field("campaign_id", &self.campaign_id)
            .field("data_subject_id", &"[REDACTED]")
            .field("version", &self.version)
            .field("visibility_label", &"[REDACTED]")
            .field("visibility_subject", &"[REDACTED]")
            .field("provenance_kind", &self.provenance_kind)
            .field("provenance_reference", &"[REDACTED]")
            .field("value_json", &"[REDACTED]")
            .field("ttl_seconds", &self.ttl_seconds)
            .finish()
    }
}

impl ProjectionCacheEntry {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        key: impl Into<String>,
        campaign_id: impl Into<String>,
        data_subject_id: impl Into<String>,
        version: i64,
        visibility_label: impl Into<String>,
        visibility_subject: impl Into<String>,
        provenance_kind: impl Into<String>,
        provenance_reference: impl Into<String>,
        value_json: impl Into<String>,
        ttl_seconds: u64,
    ) -> Result<Self, RedisProjectionError> {
        normalize_entry(&Self {
            key: key.into(),
            campaign_id: campaign_id.into(),
            data_subject_id: data_subject_id.into(),
            version,
            visibility_label: visibility_label.into(),
            visibility_subject: visibility_subject.into(),
            provenance_kind: provenance_kind.into(),
            provenance_reference: provenance_reference.into(),
            value_json: value_json.into(),
            ttl_seconds,
        })
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn campaign_id(&self) -> &str {
        &self.campaign_id
    }

    pub fn data_subject_id(&self) -> &str {
        &self.data_subject_id
    }

    pub const fn version(&self) -> i64 {
        self.version
    }

    pub fn visibility_label(&self) -> &str {
        &self.visibility_label
    }

    pub fn visibility_subject(&self) -> &str {
        &self.visibility_subject
    }

    pub fn provenance_kind(&self) -> &str {
        &self.provenance_kind
    }

    pub fn provenance_reference(&self) -> &str {
        &self.provenance_reference
    }

    pub fn value_json(&self) -> &str {
        &self.value_json
    }

    pub const fn ttl_seconds(&self) -> u64 {
        self.ttl_seconds
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RedisProjectionError {
    Configuration(&'static str),
    Unavailable,
    InvalidEntry(&'static str),
    AuthorizationDenied,
    Cryptography,
    VersionRegression,
    VersionCollision,
}

impl fmt::Display for RedisProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration(reason) => write!(formatter, "redis configuration error: {reason}"),
            Self::Unavailable => formatter.write_str("redis projection cache unavailable"),
            Self::InvalidEntry(reason) => {
                write!(formatter, "invalid projection cache entry: {reason}")
            }
            Self::AuthorizationDenied => formatter.write_str("redis projection access denied"),
            Self::Cryptography => formatter.write_str("redis projection cryptography failed"),
            Self::VersionRegression => formatter.write_str("projection version regression"),
            Self::VersionCollision => formatter.write_str("projection version collision"),
        }
    }
}

impl std::error::Error for RedisProjectionError {}

struct CacheCryptography {
    cipher: PayloadCipher,
    digest_key: Zeroizing<[u8; 32]>,
}

impl Drop for CacheCryptography {
    fn drop(&mut self) {
        self.digest_key.zeroize();
    }
}

#[derive(Clone)]
pub struct RedisProjectionCache {
    connection: ConnectionManager,
    namespace: String,
    cryptography: Arc<CacheCryptography>,
}

impl fmt::Debug for RedisProjectionCache {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RedisProjectionCache")
            .field("connection", &"[REDIS CONNECTION MANAGER]")
            .field("namespace", &self.namespace)
            .field("cryptography", &"[REDACTED]")
            .finish()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredProjectionCacheEntry {
    schema_version: u16,
    version: i64,
    ttl_seconds: u64,
    subject_index_key: String,
    entry_digest: String,
    protected_entry: Value,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProtectedProjectionCacheEntry {
    key: String,
    campaign_id: String,
    data_subject_id: String,
    visibility_label: String,
    visibility_subject: String,
    provenance_kind: String,
    provenance_reference: String,
    value_json: String,
}

impl RedisProjectionCache {
    pub async fn connect(
        redis_url: &str,
        namespace: &str,
        key_reference: &str,
        encryption_key: &[u8],
    ) -> Result<Self, RedisProjectionError> {
        Self::connect_with_tls(
            redis_url,
            namespace,
            key_reference,
            encryption_key,
            None,
            None,
            None,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn connect_with_tls(
        redis_url: &str,
        namespace: &str,
        key_reference: &str,
        encryption_key: &[u8],
        root_certificate: Option<&[u8]>,
        client_certificate: Option<&[u8]>,
        client_private_key: Option<&[u8]>,
    ) -> Result<Self, RedisProjectionError> {
        validate_redis_url(redis_url)?;
        validate_name(namespace, "namespace_required")?;
        let cryptography = Arc::new(CacheCryptography::new(key_reference, encryption_key)?);
        let client = redis_client(
            redis_url,
            root_certificate,
            client_certificate,
            client_private_key,
        )?;
        let connection = ConnectionManager::new(client)
            .await
            .map_err(|_| RedisProjectionError::Unavailable)?;
        let mut cache = Self {
            connection,
            namespace: namespace.to_owned(),
            cryptography,
        };
        cache.check_readiness().await?;
        Ok(cache)
    }

    pub async fn check_readiness(&mut self) -> Result<(), RedisProjectionError> {
        let response: String = redis::cmd("PING")
            .query_async(&mut self.connection)
            .await
            .map_err(|_| RedisProjectionError::Unavailable)?;
        if response == "PONG" {
            Ok(())
        } else {
            Err(RedisProjectionError::Unavailable)
        }
    }

    pub async fn put(&self, entry: &ProjectionCacheEntry) -> Result<(), RedisProjectionError> {
        let normalized = normalize_entry(entry)?;
        let stored = self.protect(&normalized)?;
        let encoded = serde_json::to_string(&stored)
            .map_err(|_| RedisProjectionError::InvalidEntry("serialization_failed"))?;
        let mut connection = self.connection.clone();
        let outcome: i64 = redis::Script::new(MONOTONIC_PROJECTION_SCRIPT)
            .key(self.redis_key(normalized.key()))
            .key(&stored.subject_index_key)
            .arg(normalized.version())
            .arg(encoded)
            .arg(normalized.ttl_seconds())
            .arg(&stored.entry_digest)
            .invoke_async(&mut connection)
            .await
            .map_err(|_| RedisProjectionError::Unavailable)?;
        match outcome {
            1 => Ok(()),
            -1 => Err(RedisProjectionError::VersionRegression),
            -2 => Err(RedisProjectionError::VersionCollision),
            _ => Err(RedisProjectionError::Unavailable),
        }
    }

    /// Reads only through a live, campaign-bound identity capability. Session,
    /// membership, and private-group revocation are therefore rechecked before
    /// any decrypted value can leave the cache adapter.
    pub async fn get_authorized(
        &self,
        key: &str,
        authorization: &ReplayAuthorization,
        now_unix_ms: u64,
    ) -> Result<Option<ProjectionCacheEntry>, RedisProjectionError> {
        validate_name(key, "cache_key_required")?;
        let Some(stored) = self.get_stored(key).await? else {
            return Ok(None);
        };
        validate_stored(&stored, self.subject_index_key_from_stored(&stored)?)?;
        let entry = self.unprotect(key, stored)?;
        let campaign_id = EntityId::new(entry.campaign_id())
            .map_err(|_| RedisProjectionError::InvalidEntry("stored_campaign_invalid"))?;
        let subject =
            (entry.visibility_subject() != "not_applicable").then_some(entry.visibility_subject());
        let visibility = Visibility::try_from_parts(entry.visibility_label(), subject)
            .map_err(|_| RedisProjectionError::InvalidEntry("stored_visibility_invalid"))?;
        let permitted = authorization
            .can_view(&campaign_id, &visibility, now_unix_ms)
            .map_err(|_| RedisProjectionError::AuthorizationDenied)?;
        if !permitted {
            return Err(RedisProjectionError::AuthorizationDenied);
        }
        Ok(Some(entry))
    }

    pub async fn invalidate(&self, key: &str) -> Result<(), RedisProjectionError> {
        validate_name(key, "cache_key_required")?;
        let mut connection = self.connection.clone();
        redis::Script::new(INVALIDATE_ENTRY_SCRIPT)
            .key(self.redis_key(key))
            .invoke_async::<i64>(&mut connection)
            .await
            .map(|_| ())
            .map_err(|_| RedisProjectionError::Unavailable)
    }

    /// Immediately removes all production-cache values indexed to a data
    /// subject without a global Redis SCAN.
    pub async fn invalidate_subject(
        &self,
        data_subject_id: &str,
    ) -> Result<u64, RedisProjectionError> {
        validate_name(data_subject_id, "data_subject_required")?;
        let mut connection = self.connection.clone();
        let removed = redis::Script::new(INVALIDATE_SUBJECT_SCRIPT)
            .key(self.subject_index_key(data_subject_id))
            .invoke_async::<i64>(&mut connection)
            .await
            .map_err(|_| RedisProjectionError::Unavailable)?;
        u64::try_from(removed).map_err(|_| RedisProjectionError::Unavailable)
    }

    fn protect(
        &self,
        entry: &ProjectionCacheEntry,
    ) -> Result<StoredProjectionCacheEntry, RedisProjectionError> {
        let subject_index_key = self.subject_index_key(&entry.data_subject_id);
        let protected = ProtectedProjectionCacheEntry {
            key: entry.key.clone(),
            campaign_id: entry.campaign_id.clone(),
            data_subject_id: entry.data_subject_id.clone(),
            visibility_label: entry.visibility_label.clone(),
            visibility_subject: entry.visibility_subject.clone(),
            provenance_kind: entry.provenance_kind.clone(),
            provenance_reference: entry.provenance_reference.clone(),
            value_json: entry.value_json.clone(),
        };
        let plaintext = serde_json::to_vec(&protected)
            .map_err(|_| RedisProjectionError::InvalidEntry("serialization_failed"))?;
        let version = entry.version.to_string();
        let ttl = entry.ttl_seconds.to_string();
        let aad = [
            "redis_projection_cache_v1",
            self.namespace.as_str(),
            version.as_str(),
            ttl.as_str(),
            subject_index_key.as_str(),
        ];
        let protected_entry = self
            .cryptography
            .cipher
            .encrypt_json(&plaintext, &aad)
            .map_err(|_| RedisProjectionError::Cryptography)?;
        let entry_digest = self.cryptography.digest(&plaintext, &aad)?;
        Ok(StoredProjectionCacheEntry {
            schema_version: CACHE_SCHEMA_VERSION,
            version: entry.version,
            ttl_seconds: entry.ttl_seconds,
            subject_index_key,
            entry_digest,
            protected_entry,
        })
    }

    fn unprotect(
        &self,
        requested_key: &str,
        stored: StoredProjectionCacheEntry,
    ) -> Result<ProjectionCacheEntry, RedisProjectionError> {
        let version = stored.version.to_string();
        let ttl = stored.ttl_seconds.to_string();
        let aad = [
            "redis_projection_cache_v1",
            self.namespace.as_str(),
            version.as_str(),
            ttl.as_str(),
            stored.subject_index_key.as_str(),
        ];
        let decrypted = self
            .cryptography
            .cipher
            .decrypt_json(&stored.protected_entry, &aad)
            .map_err(|_| RedisProjectionError::Cryptography)?;
        if self.cryptography.digest(decrypted.as_bytes(), &aad)? != stored.entry_digest {
            return Err(RedisProjectionError::Cryptography);
        }
        let protected: ProtectedProjectionCacheEntry = serde_json::from_slice(decrypted.as_bytes())
            .map_err(|_| RedisProjectionError::InvalidEntry("stored_entry_invalid"))?;
        if protected.key != requested_key
            || stored.subject_index_key != self.subject_index_key(&protected.data_subject_id)
        {
            return Err(RedisProjectionError::InvalidEntry("stored_binding_invalid"));
        }
        ProjectionCacheEntry::new(
            protected.key,
            protected.campaign_id,
            protected.data_subject_id,
            stored.version,
            protected.visibility_label,
            protected.visibility_subject,
            protected.provenance_kind,
            protected.provenance_reference,
            protected.value_json,
            stored.ttl_seconds,
        )
    }

    async fn get_stored(
        &self,
        key: &str,
    ) -> Result<Option<StoredProjectionCacheEntry>, RedisProjectionError> {
        let mut connection = self.connection.clone();
        let encoded: Option<String> = redis::cmd("GET")
            .arg(self.redis_key(key))
            .query_async(&mut connection)
            .await
            .map_err(|_| RedisProjectionError::Unavailable)?;
        encoded
            .map(|encoded| {
                serde_json::from_str(&encoded)
                    .map_err(|_| RedisProjectionError::InvalidEntry("stored_entry_invalid"))
            })
            .transpose()
    }

    fn subject_index_key_from_stored<'a>(
        &self,
        stored: &'a StoredProjectionCacheEntry,
    ) -> Result<&'a str, RedisProjectionError> {
        let prefix = format!("{}:subject:", self.namespace);
        if stored.subject_index_key.starts_with(&prefix)
            && stored.subject_index_key.len() == prefix.len() + 64
            && stored.subject_index_key[prefix.len()..]
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            Ok(stored.subject_index_key.as_str())
        } else {
            Err(RedisProjectionError::InvalidEntry(
                "stored_subject_index_invalid",
            ))
        }
    }

    fn redis_key(&self, key: &str) -> String {
        format!("{}:entry:{}", self.namespace, sha256_hex(key.as_bytes()))
    }

    fn subject_index_key(&self, data_subject_id: &str) -> String {
        format!(
            "{}:subject:{}",
            self.namespace,
            sha256_hex(data_subject_id.as_bytes())
        )
    }
}

fn redis_client(
    redis_url: &str,
    root_certificate: Option<&[u8]>,
    client_certificate: Option<&[u8]>,
    client_private_key: Option<&[u8]>,
) -> Result<redis::Client, RedisProjectionError> {
    let tls_fields = [
        root_certificate.is_some(),
        client_certificate.is_some(),
        client_private_key.is_some(),
    ];
    if tls_fields.iter().any(|present| *present) && !tls_fields.iter().all(|present| *present) {
        return Err(RedisProjectionError::Configuration(
            "redis_mtls_material_incomplete",
        ));
    }
    let uses_tls = Url::parse(redis_url)
        .map_err(|_| RedisProjectionError::Configuration("invalid_redis_url"))?
        .scheme()
        == "rediss";
    if uses_tls {
        let (root_certificate, client_certificate, client_private_key) = (
            root_certificate.ok_or(RedisProjectionError::Configuration(
                "redis_mtls_material_required",
            ))?,
            client_certificate.ok_or(RedisProjectionError::Configuration(
                "redis_mtls_material_required",
            ))?,
            client_private_key.ok_or(RedisProjectionError::Configuration(
                "redis_mtls_material_required",
            ))?,
        );
        redis::Client::build_with_tls(
            redis_url,
            redis::TlsCertificates {
                client_tls: Some(redis::ClientTlsConfig {
                    client_cert: client_certificate.to_vec(),
                    client_key: client_private_key.to_vec(),
                }),
                root_cert: Some(root_certificate.to_vec()),
            },
        )
        .map_err(|_| RedisProjectionError::Configuration("invalid_redis_tls_material"))
    } else {
        redis::Client::open(redis_url)
            .map_err(|_| RedisProjectionError::Configuration("invalid_redis_url"))
    }
}

impl CacheCryptography {
    fn new(key_reference: &str, encryption_key: &[u8]) -> Result<Self, RedisProjectionError> {
        if encryption_key.len() != 32 {
            return Err(RedisProjectionError::Configuration(
                "redis_cache_key_must_be_32_bytes",
            ));
        }
        let cipher = PayloadCipher::new(key_reference, encryption_key)
            .map_err(|_| RedisProjectionError::Configuration("invalid_redis_cache_key"))?;
        let mut digest_key = Zeroizing::new([0_u8; 32]);
        let mut derivation = Sha256::new();
        derivation.update(b"trpg.redis.cache.digest.v1");
        derivation.update(encryption_key);
        digest_key.copy_from_slice(&derivation.finalize());
        Ok(Self { cipher, digest_key })
    }

    fn digest(&self, plaintext: &[u8], aad: &[&str]) -> Result<String, RedisProjectionError> {
        let mut mac = HmacSha256::new_from_slice(self.digest_key.as_slice())
            .map_err(|_| RedisProjectionError::Cryptography)?;
        for field in aad {
            let length =
                u32::try_from(field.len()).map_err(|_| RedisProjectionError::Cryptography)?;
            mac.update(&length.to_be_bytes());
            mac.update(field.as_bytes());
        }
        mac.update(plaintext);
        Ok(format!(
            "hmac-sha256:{}",
            hex_bytes(&mac.finalize().into_bytes())
        ))
    }
}

fn validate_redis_url(redis_url: &str) -> Result<(), RedisProjectionError> {
    let url = Url::parse(redis_url)
        .map_err(|_| RedisProjectionError::Configuration("invalid_redis_url"))?;
    let host = url
        .host_str()
        .ok_or(RedisProjectionError::Configuration("redis_host_required"))?;
    let local = matches!(host, "localhost" | "127.0.0.1" | "::1");
    if local && matches!(url.scheme(), "redis" | "rediss") {
        return Ok(());
    }
    if !local && url.scheme() == "rediss" {
        return Ok(());
    }
    Err(RedisProjectionError::Configuration(
        "remote_redis_requires_tls",
    ))
}

fn normalize_entry(
    entry: &ProjectionCacheEntry,
) -> Result<ProjectionCacheEntry, RedisProjectionError> {
    validate_name(&entry.key, "cache_key_required")?;
    validate_name(&entry.campaign_id, "campaign_id_required")?;
    validate_name(&entry.data_subject_id, "data_subject_required")?;
    if entry.version < 0 {
        return Err(RedisProjectionError::InvalidEntry(
            "non_negative_version_required",
        ));
    }
    if entry.ttl_seconds == 0 || entry.ttl_seconds > 86_400 {
        return Err(RedisProjectionError::InvalidEntry("invalid_ttl"));
    }
    let subject =
        (entry.visibility_subject != "not_applicable").then_some(entry.visibility_subject.as_str());
    Visibility::try_from_parts(&entry.visibility_label, subject)
        .map_err(|_| RedisProjectionError::InvalidEntry("visibility_subject_mismatch"))?;
    if !matches!(
        entry.provenance_kind.as_str(),
        "user_statement"
            | "human_keeper_statement"
            | "rules_engine_decision"
            | "tool_result"
            | "agent_proposal"
            | "imported_source"
            | "system_fixture"
    ) || entry.provenance_reference.trim().is_empty()
    {
        return Err(RedisProjectionError::InvalidEntry("provenance_required"));
    }
    if entry.value_json.len() > MAX_CACHE_VALUE_BYTES {
        return Err(RedisProjectionError::InvalidEntry("value_too_large"));
    }
    let value: Value = serde_json::from_str(&entry.value_json)
        .map_err(|_| RedisProjectionError::InvalidEntry("value_must_be_json"))?;
    Ok(ProjectionCacheEntry {
        key: entry.key.clone(),
        campaign_id: entry.campaign_id.clone(),
        data_subject_id: entry.data_subject_id.clone(),
        version: entry.version,
        visibility_label: entry.visibility_label.clone(),
        visibility_subject: entry.visibility_subject.clone(),
        provenance_kind: entry.provenance_kind.clone(),
        provenance_reference: entry.provenance_reference.clone(),
        value_json: serde_json::to_string(&value)
            .map_err(|_| RedisProjectionError::InvalidEntry("value_must_be_json"))?,
        ttl_seconds: entry.ttl_seconds,
    })
}

fn validate_stored(
    stored: &StoredProjectionCacheEntry,
    subject_index_key: &str,
) -> Result<(), RedisProjectionError> {
    if stored.schema_version != CACHE_SCHEMA_VERSION
        || stored.version < 0
        || stored.ttl_seconds == 0
        || stored.ttl_seconds > 86_400
        || stored.entry_digest.len() != 76
        || !stored.entry_digest.starts_with("hmac-sha256:")
        || !stored.entry_digest[12..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || stored.subject_index_key != subject_index_key
    {
        return Err(RedisProjectionError::InvalidEntry("stored_entry_invalid"));
    }
    Ok(())
}

fn validate_name(value: &str, reason: &'static str) -> Result<(), RedisProjectionError> {
    if value.trim().is_empty() || value.len() > 256 || value.chars().any(char::is_whitespace) {
        Err(RedisProjectionError::InvalidEntry(reason))
    } else {
        Ok(())
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex_bytes(&Sha256::digest(bytes))
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn private_entry() -> ProjectionCacheEntry {
        ProjectionCacheEntry::new(
            "campaign:one:clues",
            "campaign_one",
            "player_one",
            2,
            "keeper_only",
            "not_applicable",
            "rules_engine_decision",
            "decision_secret",
            r#"{"clue":"harbor ledger"}"#,
            60,
        )
        .unwrap()
    }

    #[test]
    fn remote_plaintext_redis_is_rejected() {
        assert_eq!(
            validate_redis_url("redis://cache.example.invalid/"),
            Err(RedisProjectionError::Configuration(
                "remote_redis_requires_tls"
            ))
        );
        assert!(validate_redis_url("rediss://cache.example.invalid/").is_ok());
    }

    #[test]
    fn protected_cache_encoding_contains_no_value_or_sensitive_metadata() {
        let cache_crypto = CacheCryptography::new("redis-cache-test", &[0x42; 32]).unwrap();
        let namespace = "cache:test";
        let entry = private_entry();
        let version = entry.version().to_string();
        let ttl = entry.ttl_seconds().to_string();
        let subject_index_key = format!(
            "{namespace}:subject:{}",
            sha256_hex(entry.data_subject_id().as_bytes())
        );
        let protected = ProtectedProjectionCacheEntry {
            key: entry.key().to_owned(),
            campaign_id: entry.campaign_id().to_owned(),
            data_subject_id: entry.data_subject_id().to_owned(),
            visibility_label: entry.visibility_label().to_owned(),
            visibility_subject: entry.visibility_subject().to_owned(),
            provenance_kind: entry.provenance_kind().to_owned(),
            provenance_reference: entry.provenance_reference().to_owned(),
            value_json: entry.value_json().to_owned(),
        };
        let plaintext = serde_json::to_vec(&protected).unwrap();
        let aad = [
            "redis_projection_cache_v1",
            namespace,
            version.as_str(),
            ttl.as_str(),
            subject_index_key.as_str(),
        ];
        let envelope = cache_crypto.cipher.encrypt_json(&plaintext, &aad).unwrap();
        let entry_digest = cache_crypto.digest(&plaintext, &aad).unwrap();
        let stored = StoredProjectionCacheEntry {
            schema_version: CACHE_SCHEMA_VERSION,
            version: entry.version(),
            ttl_seconds: entry.ttl_seconds(),
            subject_index_key,
            entry_digest,
            protected_entry: envelope,
        };
        let encoded = serde_json::to_string(&stored).unwrap();
        assert!(!encoded.contains("harbor ledger"));
        assert!(!encoded.contains("player_one"));
        assert!(!encoded.contains("campaign_one"));
        assert!(!encoded.contains("keeper_only"));
        assert!(!encoded.contains("decision_secret"));
        let debug = format!("{entry:?}");
        assert!(!debug.contains("harbor ledger"));
        assert!(!debug.contains("player_one"));
        assert!(!debug.contains("keeper_only"));
        assert!(!debug.contains("decision_secret"));
    }
}
