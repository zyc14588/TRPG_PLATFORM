use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use trpg_shared_kernel::{EntityId, PrincipalScope, Visibility, VisibilityKind};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RealtimeIdentityBinding {
    pub user_id: String,
    pub campaign_id: String,
    pub seat: String,
    pub authority_mode: String,
    pub authority_epoch: u64,
}

#[derive(Clone)]
pub struct RealtimeIdentitySession {
    pool: PgPool,
    session_id: String,
    issued_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    binding: RealtimeIdentityBinding,
}

impl std::fmt::Debug for RealtimeIdentitySession {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RealtimeIdentitySession")
            .field("binding", &self.binding)
            .field("session", &"[AUTHENTICATED PERSISTED SESSION]")
            .finish()
    }
}

impl RealtimeIdentitySession {
    pub fn binding(&self) -> &RealtimeIdentityBinding {
        &self.binding
    }

    /// Rechecks session revocation, campaign membership, role, and any
    /// private-group grant in one PostgreSQL statement/MVCC snapshot.
    pub async fn can_view(
        &self,
        event_campaign_id: &EntityId,
        visibility: &Visibility,
        now_unix_ms: u64,
    ) -> Result<bool, RealtimeIdentityError> {
        if event_campaign_id.as_str() != self.binding.campaign_id
            || now_unix_ms < self.issued_at_unix_ms
            || now_unix_ms >= self.expires_at_unix_ms
        {
            return Ok(false);
        }
        let group_id = visibility.group_id().map(EntityId::as_str);
        let row = sqlx::query(
            r#"
            SELECT membership.role,
                   CASE WHEN $7::text IS NULL THEN false ELSE EXISTS (
                       SELECT 1
                         FROM campaign_group_memberships AS group_membership
                        WHERE group_membership.campaign_id = $6
                          AND group_membership.group_id = $7
                          AND group_membership.user_id = $2
                          AND group_membership.revoked_at IS NULL
                   ) END AS in_private_group
              FROM sessions AS session
              JOIN users AS account
                ON account.user_id = session.user_id
               AND account.disabled_at IS NULL
              JOIN campaign_memberships AS membership
                ON membership.campaign_id = $6
               AND membership.user_id = session.user_id
               AND membership.revoked_at IS NULL
             WHERE session.session_id = $1
               AND session.user_id = $2
               AND (extract(epoch FROM session.issued_at) * 1000)::bigint = $3
               AND (extract(epoch FROM session.expires_at) * 1000)::bigint = $4
               AND session.revoked_at IS NULL
               AND session.expires_at > to_timestamp($5::bigint / 1000.0)
            "#,
        )
        .bind(&self.session_id)
        .bind(&self.binding.user_id)
        .bind(to_i64(self.issued_at_unix_ms)?)
        .bind(to_i64(self.expires_at_unix_ms)?)
        .bind(to_i64(now_unix_ms)?)
        .bind(&self.binding.campaign_id)
        .bind(group_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| RealtimeIdentityError::Unavailable)?
        .ok_or(RealtimeIdentityError::Authorization)?;
        let principal = principal_for_role(
            &row.try_get::<String, _>("role")
                .map_err(|_| RealtimeIdentityError::InvalidData)?,
            &self.binding.user_id,
        )?;
        if visibility.label().kind() == VisibilityKind::PrivateToGroup {
            return Ok(match principal {
                PrincipalScope::Keeper | PrincipalScope::System => true,
                PrincipalScope::Player(_) | PrincipalScope::PartyMember => row
                    .try_get::<bool, _>("in_private_group")
                    .map_err(|_| RealtimeIdentityError::InvalidData)?,
                _ => false,
            });
        }
        Ok(visibility.can_view(&principal))
    }

    pub async fn can_subscribe_private_group(
        &self,
        group_id: &str,
        now_unix_ms: u64,
    ) -> Result<bool, RealtimeIdentityError> {
        let group_id = EntityId::new(group_id).map_err(|_| RealtimeIdentityError::InvalidData)?;
        let campaign_id = EntityId::new(&self.binding.campaign_id)
            .map_err(|_| RealtimeIdentityError::InvalidData)?;
        self.can_view(
            &campaign_id,
            &Visibility::private_to_group(group_id),
            now_unix_ms,
        )
        .await
    }
}

/// Least-privilege realtime identity reader. It intentionally never selects
/// password hashes or login names and never receives a database write grant.
#[derive(Clone, Debug)]
pub struct PersistentRealtimeIdentity {
    pool: PgPool,
}

impl PersistentRealtimeIdentity {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn check_readiness(&self) -> Result<(), RealtimeIdentityError> {
        sqlx::query_scalar::<_, i32>("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .map(|_| ())
            .map_err(|_| RealtimeIdentityError::Unavailable)
    }

    pub async fn authenticate(
        &self,
        token: Option<&str>,
        campaign_id: &str,
        now_unix_ms: u64,
    ) -> Result<RealtimeIdentitySession, RealtimeIdentityError> {
        validate_identifier(campaign_id)?;
        let token = token
            .filter(|token| !token.is_empty() && token.len() <= 2_048)
            .ok_or(RealtimeIdentityError::Authentication)?;
        let token_hash = Sha256::digest(token.as_bytes()).to_vec();
        let row = sqlx::query(
            r#"
            SELECT session.session_id, session.user_id,
                   (extract(epoch FROM session.issued_at) * 1000)::bigint AS issued_at_unix_ms,
                   (extract(epoch FROM session.expires_at) * 1000)::bigint AS expires_at_unix_ms,
                   membership.role, authority.authority_mode,
                   authority.contract_version
              FROM sessions AS session
              JOIN users AS account
                ON account.user_id = session.user_id
               AND account.disabled_at IS NULL
              JOIN campaign_memberships AS membership
                ON membership.campaign_id = $2
               AND membership.user_id = session.user_id
               AND membership.revoked_at IS NULL
              JOIN authority_contracts AS authority
                ON authority.campaign_id = membership.campaign_id
             WHERE session.token_hash = $1
               AND session.revoked_at IS NULL
               AND session.expires_at > to_timestamp($3::bigint / 1000.0)
            "#,
        )
        .bind(token_hash)
        .bind(campaign_id)
        .bind(to_i64(now_unix_ms)?)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| RealtimeIdentityError::Unavailable)?
        .ok_or(RealtimeIdentityError::Authentication)?;
        session_from_row(self.pool.clone(), campaign_id, row)
    }

    pub async fn reauthorize(
        &self,
        session: &mut RealtimeIdentitySession,
        now_unix_ms: u64,
    ) -> Result<RealtimeIdentityBinding, RealtimeIdentityError> {
        let row = sqlx::query(
            r#"
            SELECT membership.role, authority.authority_mode,
                   authority.contract_version
              FROM sessions AS session
              JOIN users AS account
                ON account.user_id = session.user_id
               AND account.disabled_at IS NULL
              JOIN campaign_memberships AS membership
                ON membership.campaign_id = $6
               AND membership.user_id = session.user_id
               AND membership.revoked_at IS NULL
              JOIN authority_contracts AS authority
                ON authority.campaign_id = membership.campaign_id
             WHERE session.session_id = $1
               AND session.user_id = $2
               AND (extract(epoch FROM session.issued_at) * 1000)::bigint = $3
               AND (extract(epoch FROM session.expires_at) * 1000)::bigint = $4
               AND session.revoked_at IS NULL
               AND session.expires_at > to_timestamp($5::bigint / 1000.0)
            "#,
        )
        .bind(&session.session_id)
        .bind(&session.binding.user_id)
        .bind(to_i64(session.issued_at_unix_ms)?)
        .bind(to_i64(session.expires_at_unix_ms)?)
        .bind(to_i64(now_unix_ms)?)
        .bind(&session.binding.campaign_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| RealtimeIdentityError::Unavailable)?
        .ok_or(RealtimeIdentityError::Authorization)?;
        session.binding.seat = normalize_role(
            &row.try_get::<String, _>("role")
                .map_err(|_| RealtimeIdentityError::InvalidData)?,
        )?
        .to_owned();
        session.binding.authority_mode = normalize_authority(
            &row.try_get::<String, _>("authority_mode")
                .map_err(|_| RealtimeIdentityError::InvalidData)?,
        )?
        .to_owned();
        session.binding.authority_epoch = u64::try_from(
            row.try_get::<i64, _>("contract_version")
                .map_err(|_| RealtimeIdentityError::InvalidData)?,
        )
        .map_err(|_| RealtimeIdentityError::InvalidData)?;
        if session.binding.authority_epoch == 0 {
            return Err(RealtimeIdentityError::InvalidData);
        }
        Ok(session.binding.clone())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RealtimeIdentityError {
    Authentication,
    Authorization,
    Unavailable,
    InvalidData,
}

fn session_from_row(
    pool: PgPool,
    campaign_id: &str,
    row: sqlx::postgres::PgRow,
) -> Result<RealtimeIdentitySession, RealtimeIdentityError> {
    let authority_epoch = u64::try_from(
        row.try_get::<i64, _>("contract_version")
            .map_err(|_| RealtimeIdentityError::InvalidData)?,
    )
    .map_err(|_| RealtimeIdentityError::InvalidData)?;
    if authority_epoch == 0 {
        return Err(RealtimeIdentityError::InvalidData);
    }
    Ok(RealtimeIdentitySession {
        pool,
        session_id: row
            .try_get("session_id")
            .map_err(|_| RealtimeIdentityError::InvalidData)?,
        issued_at_unix_ms: from_i64(
            row.try_get("issued_at_unix_ms")
                .map_err(|_| RealtimeIdentityError::InvalidData)?,
        )?,
        expires_at_unix_ms: from_i64(
            row.try_get("expires_at_unix_ms")
                .map_err(|_| RealtimeIdentityError::InvalidData)?,
        )?,
        binding: RealtimeIdentityBinding {
            user_id: row
                .try_get("user_id")
                .map_err(|_| RealtimeIdentityError::InvalidData)?,
            campaign_id: campaign_id.to_owned(),
            seat: normalize_role(
                &row.try_get::<String, _>("role")
                    .map_err(|_| RealtimeIdentityError::InvalidData)?,
            )?
            .to_owned(),
            authority_mode: normalize_authority(
                &row.try_get::<String, _>("authority_mode")
                    .map_err(|_| RealtimeIdentityError::InvalidData)?,
            )?
            .to_owned(),
            authority_epoch,
        },
    })
}

fn principal_for_role(role: &str, user_id: &str) -> Result<PrincipalScope, RealtimeIdentityError> {
    Ok(match normalize_role(role)? {
        "human_kp" => PrincipalScope::Keeper,
        "player" => PrincipalScope::Player(
            EntityId::new(user_id).map_err(|_| RealtimeIdentityError::InvalidData)?,
        ),
        "campaign_owner" => PrincipalScope::PartyMember,
        "spectator" => PrincipalScope::Spectator,
        _ => return Err(RealtimeIdentityError::InvalidData),
    })
}

fn normalize_role(value: &str) -> Result<&'static str, RealtimeIdentityError> {
    match value {
        "CAMPAIGN_OWNER" => Ok("campaign_owner"),
        "HUMAN_KEEPER" => Ok("human_kp"),
        "PLAYER" => Ok("player"),
        "SPECTATOR" => Ok("spectator"),
        _ => Err(RealtimeIdentityError::InvalidData),
    }
}

fn normalize_authority(value: &str) -> Result<&'static str, RealtimeIdentityError> {
    match value {
        "HUMAN_KP" => Ok("human_kp"),
        "AI_KP" => Ok("ai_kp"),
        _ => Err(RealtimeIdentityError::InvalidData),
    }
}

fn validate_identifier(value: &str) -> Result<(), RealtimeIdentityError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(RealtimeIdentityError::InvalidData);
    }
    Ok(())
}

fn to_i64(value: u64) -> Result<i64, RealtimeIdentityError> {
    i64::try_from(value).map_err(|_| RealtimeIdentityError::InvalidData)
}

fn from_i64(value: i64) -> Result<u64, RealtimeIdentityError> {
    u64::try_from(value).map_err(|_| RealtimeIdentityError::InvalidData)
}
