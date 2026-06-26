use async_trait::async_trait;
use auth::{
    AuditLog, AuditLogRepository, AuthContext, EmailAddress, IdempotencyCheck, IdempotencyRecord,
    IdempotencyRepository, IdempotencyStatus, IdentityRepository, RefreshSession, RefreshSessionId,
    RefreshSessionRepository, RefreshSessionStatus, RepositoryError, RepositoryTransaction, Room,
    RoomId, RoomInvite, RoomMember, RoomRepository, RoomRole, TokenHash, TransactionalRepository,
    User, UserId,
};
use serde_json::Value;
use sqlx::{postgres::PgPoolOptions, PgPool, Postgres, Row, Transaction};
use std::{
    str::FromStr,
    time::{Duration, SystemTime},
};

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations");

#[derive(Debug, Clone)]
pub struct PostgresRepositories {
    pool: PgPool,
}

impl PostgresRepositories {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn connect(database_url: &str) -> Result<Self, RepositoryError> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await
            .map_err(map_sqlx)?;
        Ok(Self::new(pool))
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn set_rls_context(
        tx: &mut Transaction<'_, Postgres>,
        ctx: &AuthContext,
    ) -> Result<(), RepositoryError> {
        let room_id = ctx.room_id.map(|id| id.to_string()).unwrap_or_default();
        sqlx::query("SELECT set_config('app.user_id', $1, true)")
            .bind(ctx.user_id.to_string())
            .execute(&mut **tx)
            .await
            .map_err(map_sqlx)?;
        sqlx::query("SELECT set_config('app.room_id', $1, true)")
            .bind(room_id)
            .execute(&mut **tx)
            .await
            .map_err(map_sqlx)?;
        sqlx::query("SELECT set_config('app.room_role', $1, true)")
            .bind(ctx.role.as_str())
            .execute(&mut **tx)
            .await
            .map_err(map_sqlx)?;
        Ok(())
    }
}

pub struct PostgresRepositoryTransaction<'a> {
    tx: Option<Transaction<'a, Postgres>>,
}

#[async_trait]
impl RepositoryTransaction for PostgresRepositoryTransaction<'_> {
    async fn commit(mut self: Box<Self>) -> Result<(), RepositoryError> {
        if let Some(tx) = self.tx.take() {
            tx.commit().await.map_err(map_sqlx)?;
        }
        Ok(())
    }

    async fn rollback(mut self: Box<Self>) -> Result<(), RepositoryError> {
        if let Some(tx) = self.tx.take() {
            tx.rollback().await.map_err(map_sqlx)?;
        }
        Ok(())
    }
}

#[async_trait]
impl TransactionalRepository for PostgresRepositories {
    async fn begin_transaction(
        &self,
    ) -> Result<Box<dyn RepositoryTransaction + Send + '_>, RepositoryError> {
        let tx = self.pool.begin().await.map_err(map_sqlx)?;
        Ok(Box::new(PostgresRepositoryTransaction { tx: Some(tx) }))
    }
}

#[async_trait]
impl IdentityRepository for PostgresRepositories {
    async fn upsert_user(&self, user: &User) -> Result<(), RepositoryError> {
        sqlx::query(
            r#"
            INSERT INTO users (id, email, display_name)
            VALUES ($1, $2, $3)
            ON CONFLICT (email)
            DO UPDATE SET display_name = EXCLUDED.display_name, updated_at = now()
            "#,
        )
        .bind(user.id.0)
        .bind(user.email.as_str())
        .bind(&user.display_name)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    async fn find_user_by_email(
        &self,
        email: &EmailAddress,
    ) -> Result<Option<User>, RepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT id, email, display_name
            FROM users
            WHERE email = $1
            "#,
        )
        .bind(email.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;

        row.map(user_from_row).transpose()
    }
}

#[async_trait]
impl RefreshSessionRepository for PostgresRepositories {
    async fn create_refresh_session(
        &self,
        session: &RefreshSession,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            r#"
            INSERT INTO refresh_sessions (
                id, user_id, session_family_id, current_token_hash, previous_token_hash,
                status, expires_at, rotated_at, revoked_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, to_timestamp($7), NULL, NULL)
            "#,
        )
        .bind(session.id.0)
        .bind(session.user_id.0)
        .bind(session.session_family_id)
        .bind(session.current_token_hash.as_str())
        .bind(session.previous_token_hash.as_ref().map(TokenHash::as_str))
        .bind(session.status.as_str())
        .bind(unix_seconds(session.expires_at)?)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    async fn save_refresh_session(&self, session: &RefreshSession) -> Result<(), RepositoryError> {
        sqlx::query(
            r#"
            UPDATE refresh_sessions
            SET current_token_hash = $2,
                previous_token_hash = $3,
                status = $4,
                expires_at = to_timestamp($5),
                rotated_at = CASE WHEN $6::bigint IS NULL THEN NULL ELSE to_timestamp($6) END,
                revoked_at = CASE WHEN $7::bigint IS NULL THEN NULL ELSE to_timestamp($7) END,
                updated_at = now()
            WHERE id = $1
            "#,
        )
        .bind(session.id.0)
        .bind(session.current_token_hash.as_str())
        .bind(session.previous_token_hash.as_ref().map(TokenHash::as_str))
        .bind(session.status.as_str())
        .bind(unix_seconds(session.expires_at)?)
        .bind(optional_unix_seconds(session.rotated_at)?)
        .bind(optional_unix_seconds(session.revoked_at)?)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    async fn find_refresh_session_by_token_hash(
        &self,
        token_hash: &TokenHash,
    ) -> Result<Option<RefreshSession>, RepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT id,
                   user_id,
                   session_family_id,
                   current_token_hash,
                   previous_token_hash,
                   status,
                   extract(epoch from expires_at)::bigint AS expires_at_epoch,
                   extract(epoch from rotated_at)::bigint AS rotated_at_epoch,
                   extract(epoch from revoked_at)::bigint AS revoked_at_epoch
            FROM refresh_sessions
            WHERE current_token_hash = $1 OR previous_token_hash = $1
            "#,
        )
        .bind(token_hash.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;

        row.map(refresh_session_from_row).transpose()
    }
}

#[async_trait]
impl RoomRepository for PostgresRepositories {
    async fn create_room(&self, room: &Room) -> Result<(), RepositoryError> {
        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;
        sqlx::query(
            r#"
            INSERT INTO rooms (id, owner_id, title, system_name, privacy_mode, version)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(room.id.0)
        .bind(room.owner_id.0)
        .bind(&room.title)
        .bind(&room.system_name)
        .bind(room.privacy_mode.as_str())
        .bind(room.version)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;

        sqlx::query(
            r#"
            INSERT INTO room_members (room_id, user_id, role)
            VALUES ($1, $2, 'owner')
            ON CONFLICT (room_id, user_id) DO UPDATE SET role = 'owner', updated_at = now()
            "#,
        )
        .bind(room.id.0)
        .bind(room.owner_id.0)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;

        tx.commit().await.map_err(map_sqlx)?;
        Ok(())
    }

    async fn add_room_member(&self, member: &RoomMember) -> Result<(), RepositoryError> {
        sqlx::query(
            r#"
            INSERT INTO room_members (room_id, user_id, role)
            VALUES ($1, $2, $3)
            ON CONFLICT (room_id, user_id)
            DO UPDATE SET role = EXCLUDED.role, updated_at = now()
            "#,
        )
        .bind(member.room_id.0)
        .bind(member.user_id.0)
        .bind(member.role.as_str())
        .execute(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    async fn get_room_member(
        &self,
        room_id: RoomId,
        user_id: UserId,
    ) -> Result<Option<RoomMember>, RepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT room_id, user_id, role
            FROM room_members
            WHERE room_id = $1 AND user_id = $2
            "#,
        )
        .bind(room_id.0)
        .bind(user_id.0)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;

        row.map(room_member_from_row).transpose()
    }

    async fn create_room_invite(&self, invite: &RoomInvite) -> Result<(), RepositoryError> {
        sqlx::query(
            r#"
            INSERT INTO room_invites (
                id, room_id, invited_email, invited_role, token_hash, status,
                invited_by, accepted_by, expires_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, to_timestamp($9))
            "#,
        )
        .bind(invite.id.0)
        .bind(invite.room_id.0)
        .bind(invite.invited_email.as_str())
        .bind(invite.role.as_str())
        .bind(invite.token_hash.as_str())
        .bind(invite.status.as_str())
        .bind(invite.invited_by.0)
        .bind(invite.accepted_by.map(|id| id.0))
        .bind(unix_seconds(invite.expires_at)?)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    async fn save_room_invite(&self, invite: &RoomInvite) -> Result<(), RepositoryError> {
        sqlx::query(
            r#"
            UPDATE room_invites
            SET status = $2,
                accepted_by = $3,
                expires_at = to_timestamp($4),
                updated_at = now()
            WHERE id = $1
            "#,
        )
        .bind(invite.id.0)
        .bind(invite.status.as_str())
        .bind(invite.accepted_by.map(|id| id.0))
        .bind(unix_seconds(invite.expires_at)?)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }
}

#[async_trait]
impl AuditLogRepository for PostgresRepositories {
    async fn append_audit_log(&self, log: &AuditLog) -> Result<(), RepositoryError> {
        sqlx::query(
            r#"
            INSERT INTO audit_logs (
                room_id, actor_id, action, target_type, target_id, scope,
                payload_json, request_id, outcome
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(log.room_id.map(|id| id.0))
        .bind(log.actor_id.map(|id| id.0))
        .bind(&log.action)
        .bind(&log.target_type)
        .bind(log.target_id)
        .bind(log.scope.as_str())
        .bind(&log.payload_json)
        .bind(log.request_id)
        .bind(log.outcome.as_str())
        .execute(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }
}

#[async_trait]
impl IdempotencyRepository for PostgresRepositories {
    async fn claim_idempotency_key(
        &self,
        record: &IdempotencyRecord,
        ttl: Duration,
    ) -> Result<IdempotencyCheck, RepositoryError> {
        sqlx::query(
            "DELETE FROM idempotency_keys WHERE scope = $1 AND key = $2 AND expires_at <= now()",
        )
        .bind(&record.scope)
        .bind(&record.key)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx)?;

        let expires_at = unix_seconds(SystemTime::now() + ttl)?;
        let inserted = sqlx::query_scalar::<_, String>(
            r#"
            INSERT INTO idempotency_keys (scope, key, request_hash, status, response_json, expires_at)
            VALUES ($1, $2, $3, $4, $5, to_timestamp($6))
            ON CONFLICT (scope, key) DO NOTHING
            RETURNING key
            "#,
        )
        .bind(&record.scope)
        .bind(&record.key)
        .bind(&record.request_hash)
        .bind(record.status.as_str())
        .bind(&record.response_json)
        .bind(expires_at)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;

        if inserted.is_some() {
            return Ok(IdempotencyCheck::Claimed);
        }

        let row = sqlx::query(
            r#"
            SELECT scope, key, request_hash, status, response_json
            FROM idempotency_keys
            WHERE scope = $1 AND key = $2
            "#,
        )
        .bind(&record.scope)
        .bind(&record.key)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx)?;

        let existing = idempotency_record_from_row(row)?;
        if existing.request_hash == record.request_hash {
            Ok(IdempotencyCheck::Duplicate(existing))
        } else {
            Ok(IdempotencyCheck::Conflict)
        }
    }
}

fn map_sqlx(error: sqlx::Error) -> RepositoryError {
    if let sqlx::Error::Database(database_error) = &error {
        if let Some(code) = database_error.code() {
            if game_core::is_retryable_sqlstate(code.as_ref()) {
                return RepositoryError::RetryableDb(code.to_string());
            }
            if code == "23505" {
                return RepositoryError::Duplicate;
            }
        }
    }
    RepositoryError::Database("database error".to_owned())
}

#[cfg(test)]
fn map_migrate(error: sqlx::migrate::MigrateError) -> RepositoryError {
    RepositoryError::Database(error.to_string())
}

fn user_from_row(row: sqlx::postgres::PgRow) -> Result<User, RepositoryError> {
    let email: String = row.try_get("email").map_err(map_sqlx)?;
    Ok(User {
        id: UserId(row.try_get("id").map_err(map_sqlx)?),
        email: EmailAddress::parse(email)
            .map_err(|err| RepositoryError::Database(err.to_string()))?,
        display_name: row.try_get("display_name").map_err(map_sqlx)?,
    })
}

fn room_member_from_row(row: sqlx::postgres::PgRow) -> Result<RoomMember, RepositoryError> {
    let role: String = row.try_get("role").map_err(map_sqlx)?;
    Ok(RoomMember {
        room_id: RoomId(row.try_get("room_id").map_err(map_sqlx)?),
        user_id: UserId(row.try_get("user_id").map_err(map_sqlx)?),
        role: RoomRole::from_str(&role)
            .map_err(|err| RepositoryError::Database(err.to_string()))?,
    })
}

fn refresh_session_from_row(row: sqlx::postgres::PgRow) -> Result<RefreshSession, RepositoryError> {
    let current_hash: String = row.try_get("current_token_hash").map_err(map_sqlx)?;
    let previous_hash: Option<String> = row.try_get("previous_token_hash").map_err(map_sqlx)?;
    let status: String = row.try_get("status").map_err(map_sqlx)?;
    let expires_at_epoch: i64 = row.try_get("expires_at_epoch").map_err(map_sqlx)?;
    let rotated_at_epoch: Option<i64> = row.try_get("rotated_at_epoch").map_err(map_sqlx)?;
    let revoked_at_epoch: Option<i64> = row.try_get("revoked_at_epoch").map_err(map_sqlx)?;

    Ok(RefreshSession {
        id: RefreshSessionId(row.try_get("id").map_err(map_sqlx)?),
        user_id: UserId(row.try_get("user_id").map_err(map_sqlx)?),
        session_family_id: row.try_get("session_family_id").map_err(map_sqlx)?,
        current_token_hash: TokenHash::new(current_hash)
            .map_err(|err| RepositoryError::Database(err.to_string()))?,
        previous_token_hash: previous_hash
            .map(TokenHash::new)
            .transpose()
            .map_err(|err| RepositoryError::Database(err.to_string()))?,
        status: refresh_status_from_str(&status)?,
        expires_at: from_unix_seconds(expires_at_epoch)?,
        rotated_at: rotated_at_epoch.map(from_unix_seconds).transpose()?,
        revoked_at: revoked_at_epoch.map(from_unix_seconds).transpose()?,
    })
}

fn idempotency_record_from_row(
    row: sqlx::postgres::PgRow,
) -> Result<IdempotencyRecord, RepositoryError> {
    let status: String = row.try_get("status").map_err(map_sqlx)?;
    Ok(IdempotencyRecord {
        scope: row.try_get("scope").map_err(map_sqlx)?,
        key: row.try_get("key").map_err(map_sqlx)?,
        request_hash: row.try_get("request_hash").map_err(map_sqlx)?,
        status: idempotency_status_from_str(&status)?,
        response_json: row
            .try_get::<Option<Value>, _>("response_json")
            .map_err(map_sqlx)?,
    })
}

fn refresh_status_from_str(value: &str) -> Result<RefreshSessionStatus, RepositoryError> {
    match value {
        "active" => Ok(RefreshSessionStatus::Active),
        "revoked" => Ok(RefreshSessionStatus::Revoked),
        "expired" => Ok(RefreshSessionStatus::Expired),
        _ => Err(RepositoryError::Database(format!(
            "unknown refresh session status: {value}"
        ))),
    }
}

fn idempotency_status_from_str(value: &str) -> Result<IdempotencyStatus, RepositoryError> {
    match value {
        "in_progress" => Ok(IdempotencyStatus::InProgress),
        "completed" => Ok(IdempotencyStatus::Completed),
        "failed" => Ok(IdempotencyStatus::Failed),
        _ => Err(RepositoryError::Database(format!(
            "unknown idempotency status: {value}"
        ))),
    }
}

fn unix_seconds(value: SystemTime) -> Result<i64, RepositoryError> {
    let duration = value
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|err| RepositoryError::Database(err.to_string()))?;
    i64::try_from(duration.as_secs()).map_err(|err| RepositoryError::Database(err.to_string()))
}

fn optional_unix_seconds(value: Option<SystemTime>) -> Result<Option<i64>, RepositoryError> {
    value.map(unix_seconds).transpose()
}

fn from_unix_seconds(value: i64) -> Result<SystemTime, RepositoryError> {
    let seconds = u64::try_from(value).map_err(|err| RepositoryError::Database(err.to_string()))?;
    Ok(SystemTime::UNIX_EPOCH + Duration::from_secs(seconds))
}

#[cfg(test)]
mod tests {
    use super::*;
    use auth::{
        AuditOutcome, AuthzDecision, MagicLinkPort, MagicLinkRequest, MockAuthProvider, RoomAction,
        RoomPrivacyMode, VisibilityScope,
    };
    use sqlx::AssertSqlSafe;
    use sqlx::Executor;
    use uuid::Uuid;

    async fn maybe_test_pool() -> Option<PgPool> {
        let database_url = std::env::var("DATABASE_URL").ok()?;
        PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await
            .ok()
    }

    fn email(value: &str) -> Result<EmailAddress, RepositoryError> {
        EmailAddress::parse(value).map_err(|err| RepositoryError::Database(err.to_string()))
    }

    fn token_hash(value: &str) -> Result<TokenHash, RepositoryError> {
        TokenHash::new(value).map_err(|err| RepositoryError::Database(err.to_string()))
    }

    fn user(display: &str) -> Result<User, RepositoryError> {
        let id = Uuid::new_v4();
        Ok(User {
            id: UserId(id),
            email: email(&format!("{id}@example.test"))?,
            display_name: display.to_owned(),
        })
    }

    fn room(owner_id: UserId) -> Room {
        Room {
            id: RoomId(Uuid::new_v4()),
            owner_id,
            title: "test room".to_owned(),
            system_name: "generic_percentile".to_owned(),
            privacy_mode: RoomPrivacyMode::PrivateHybrid,
            version: 0,
        }
    }

    async fn migrated_repo() -> Option<PostgresRepositories> {
        let pool = maybe_test_pool().await?;
        if MIGRATOR.run(&pool).await.is_err() {
            return None;
        }
        Some(PostgresRepositories::new(pool))
    }

    async fn ensure_rls_test_role(repo: &PostgresRepositories) -> Result<(), RepositoryError> {
        repo.pool()
            .execute(
                r#"
                DO $$
                BEGIN
                    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_rls_test') THEN
                        CREATE ROLE trpg_rls_test;
                    END IF;
                END
                $$;
                "#,
            )
            .await
            .map_err(map_sqlx)?;
        repo.pool()
            .execute("GRANT USAGE ON SCHEMA public, app TO trpg_rls_test")
            .await
            .map_err(map_sqlx)?;
        repo.pool()
            .execute("GRANT SELECT ON ALL TABLES IN SCHEMA public TO trpg_rls_test")
            .await
            .map_err(map_sqlx)?;
        Ok(())
    }

    #[tokio::test]
    async fn duplicate_idempotency_key_is_detected() -> Result<(), RepositoryError> {
        let Some(repo) = migrated_repo().await else {
            return Ok(());
        };
        let scope = format!("test:{}", Uuid::new_v4());
        let first = IdempotencyRecord {
            scope: scope.clone(),
            key: "same-key".to_owned(),
            request_hash: "hash-a".to_owned(),
            status: IdempotencyStatus::InProgress,
            response_json: None,
        };
        let second = first.clone();

        let first_result = repo
            .claim_idempotency_key(&first, Duration::from_secs(60))
            .await?;
        let second_result = repo
            .claim_idempotency_key(&second, Duration::from_secs(60))
            .await?;

        assert_eq!(first_result, IdempotencyCheck::Claimed);
        assert!(matches!(second_result, IdempotencyCheck::Duplicate(_)));
        Ok(())
    }

    #[tokio::test]
    async fn refresh_reuse_can_be_detected_after_persisted_rotation() -> Result<(), RepositoryError>
    {
        let Some(repo) = migrated_repo().await else {
            return Ok(());
        };
        let owner = user("Owner")?;
        repo.upsert_user(&owner).await?;
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
        let old_hash = token_hash("old_refresh_hash")?;
        let mut session =
            RefreshSession::active(owner.id, old_hash.clone(), now, Duration::from_secs(3_600));
        repo.create_refresh_session(&session).await?;

        session
            .rotate(
                &old_hash,
                token_hash("new_refresh_hash")?,
                now + Duration::from_secs(1),
            )
            .map_err(|err| RepositoryError::Database(err.to_string()))?;
        repo.save_refresh_session(&session).await?;

        let loaded = repo
            .find_refresh_session_by_token_hash(&old_hash)
            .await?
            .ok_or(RepositoryError::NotFound)?;
        let mut loaded = loaded;
        let reuse_result = loaded.rotate(
            &old_hash,
            token_hash("reused_refresh_hash")?,
            now + Duration::from_secs(2),
        );

        assert!(matches!(
            reuse_result,
            Err(auth::RefreshSessionError::ReuseDetected)
        ));
        assert_eq!(loaded.status, RefreshSessionStatus::Revoked);
        Ok(())
    }

    #[tokio::test]
    async fn repository_get_room_member_is_room_scoped() -> Result<(), RepositoryError> {
        let Some(repo) = migrated_repo().await else {
            return Ok(());
        };
        let owner = user("Owner")?;
        let pl = user("PL")?;
        repo.upsert_user(&owner).await?;
        repo.upsert_user(&pl).await?;
        let room_a = room(owner.id);
        let room_b = room(owner.id);
        repo.create_room(&room_a).await?;
        repo.create_room(&room_b).await?;
        repo.add_room_member(&RoomMember {
            room_id: room_a.id,
            user_id: pl.id,
            role: RoomRole::Pl,
        })
        .await?;

        let in_room = repo.get_room_member(room_a.id, pl.id).await?;
        let cross_room = repo.get_room_member(room_b.id, pl.id).await?;

        assert!(in_room.is_some());
        assert!(cross_room.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn audit_records_success_and_failure() -> Result<(), RepositoryError> {
        let Some(repo) = migrated_repo().await else {
            return Ok(());
        };
        let owner = user("Owner")?;
        repo.upsert_user(&owner).await?;
        let room = room(owner.id);
        repo.create_room(&room).await?;

        for outcome in [AuditOutcome::Success, AuditOutcome::Failure] {
            repo.append_audit_log(&AuditLog {
                room_id: Some(room.id),
                actor_id: Some(owner.id),
                action: "phase_1a.audit_test".to_owned(),
                target_type: "room".to_owned(),
                target_id: Some(room.id.0),
                scope: VisibilityScope::SystemInternal,
                outcome,
                payload_json: serde_json::json!({ "ok": outcome == AuditOutcome::Success }),
                request_id: Some(Uuid::new_v4()),
            })
            .await?;
        }

        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM audit_logs WHERE room_id = $1 AND action = 'phase_1a.audit_test'",
        )
        .bind(room.id.0)
        .fetch_one(repo.pool())
        .await
        .map_err(map_sqlx)?;

        assert_eq!(count, 2);
        Ok(())
    }

    #[tokio::test]
    async fn pure_backend_vertical_flow_without_http() -> Result<(), RepositoryError> {
        let Some(repo) = migrated_repo().await else {
            return Ok(());
        };
        ensure_rls_test_role(&repo).await?;

        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(2_000);
        let auth_provider = MockAuthProvider;
        let owner_challenge = auth_provider
            .issue_magic_link(
                MagicLinkRequest {
                    email: email("owner@example.test")?,
                    redirect_uri: "http://localhost/auth/callback".to_owned(),
                },
                now,
            )
            .await
            .map_err(|err| RepositoryError::Database(err.to_string()))?;
        let owner_identity = auth_provider
            .verify_magic_link(
                &owner_challenge,
                &owner_challenge.token_hash,
                now + Duration::from_secs(1),
            )
            .await
            .map_err(|err| RepositoryError::Database(err.to_string()))?;
        let owner = User {
            id: UserId(Uuid::new_v4()),
            email: owner_identity.email,
            display_name: owner_identity.display_name,
        };
        let pl = user("PL")?;

        repo.upsert_user(&owner).await?;
        repo.upsert_user(&pl).await?;

        let refresh = RefreshSession::active(
            owner.id,
            token_hash("vertical_refresh_hash")?,
            now,
            Duration::from_secs(3_600),
        );
        repo.create_refresh_session(&refresh).await?;

        let room = room(owner.id);
        repo.create_room(&room).await?;
        let mut invite = RoomInvite {
            id: auth::RoomInviteId(Uuid::new_v4()),
            room_id: room.id,
            invited_email: pl.email.clone(),
            role: RoomRole::Pl,
            token_hash: token_hash("vertical_invite_hash")?,
            status: auth::RoomInviteStatus::Pending,
            invited_by: owner.id,
            accepted_by: None,
            expires_at: now + Duration::from_secs(600),
        };
        repo.create_room_invite(&invite).await?;
        invite
            .accept(pl.id, now + Duration::from_secs(2))
            .map_err(|err| RepositoryError::Database(err.to_string()))?;
        repo.save_room_invite(&invite).await?;
        repo.add_room_member(&RoomMember {
            room_id: room.id,
            user_id: pl.id,
            role: RoomRole::Pl,
        })
        .await?;

        let idempotency = repo
            .claim_idempotency_key(
                &IdempotencyRecord {
                    scope: format!("room:{}", room.id.0),
                    key: "vertical-flow".to_owned(),
                    request_hash: "vertical-hash".to_owned(),
                    status: IdempotencyStatus::InProgress,
                    response_json: None,
                },
                Duration::from_secs(60),
            )
            .await?;
        assert_eq!(idempotency, IdempotencyCheck::Claimed);

        repo.append_audit_log(&AuditLog {
            room_id: Some(room.id),
            actor_id: Some(owner.id),
            action: "phase_1a.vertical_flow".to_owned(),
            target_type: "room".to_owned(),
            target_id: Some(room.id.0),
            scope: VisibilityScope::SystemInternal,
            outcome: AuditOutcome::Success,
            payload_json: serde_json::json!({ "http": false }),
            request_id: Some(Uuid::new_v4()),
        })
        .await?;

        let mut tx = repo.pool().begin().await.map_err(map_sqlx)?;
        sqlx::query("SET LOCAL ROLE trpg_rls_test")
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx)?;
        PostgresRepositories::set_rls_context(
            &mut tx,
            &AuthContext {
                user_id: pl.id.0,
                room_id: Some(room.id.0),
                role: RoomRole::Pl,
            },
        )
        .await?;

        let visible_count: i64 = sqlx::query_scalar("SELECT count(*) FROM rooms WHERE id = $1")
            .bind(room.id.0)
            .fetch_one(&mut *tx)
            .await
            .map_err(map_sqlx)?;
        tx.rollback().await.map_err(map_sqlx)?;

        assert_eq!(visible_count, 1);
        Ok(())
    }

    #[tokio::test]
    async fn owner_kp_pl_and_spectator_authorization_differs() {
        let user_id = Uuid::new_v4();
        let room_id = Uuid::new_v4();
        let roles = [
            (RoomRole::Owner, true, true),
            (RoomRole::Kp, false, true),
            (RoomRole::Pl, false, false),
            (RoomRole::Observer, false, false),
        ];

        for (role, manage_room, kp_content) in roles {
            let ctx = AuthContext {
                user_id,
                room_id: Some(room_id),
                role,
            };
            assert_eq!(
                auth::authorize_room_action(
                    &ctx,
                    RoomAction::ManageRoom,
                    RoomPrivacyMode::Standard
                ),
                if manage_room {
                    AuthzDecision::Allow
                } else {
                    AuthzDecision::Deny
                }
            );
            assert_eq!(ctx.can_view(VisibilityScope::KpSecret), kp_content);
        }
    }

    #[tokio::test]
    async fn postgres_rls_rejects_non_member() -> Result<(), RepositoryError> {
        let Some(repo) = migrated_repo().await else {
            return Ok(());
        };
        ensure_rls_test_role(&repo).await?;

        let owner = user("Owner")?;
        let outsider = user("Outsider")?;
        repo.upsert_user(&owner).await?;
        repo.upsert_user(&outsider).await?;
        let room = room(owner.id);
        repo.create_room(&room).await?;

        let mut tx = repo.pool().begin().await.map_err(map_sqlx)?;
        sqlx::query("SET LOCAL ROLE trpg_rls_test")
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx)?;
        PostgresRepositories::set_rls_context(
            &mut tx,
            &AuthContext {
                user_id: outsider.id.0,
                room_id: Some(room.id.0),
                role: RoomRole::Pl,
            },
        )
        .await?;

        let visible_count: i64 = sqlx::query_scalar("SELECT count(*) FROM rooms WHERE id = $1")
            .bind(room.id.0)
            .fetch_one(&mut *tx)
            .await
            .map_err(map_sqlx)?;

        tx.rollback().await.map_err(map_sqlx)?;
        assert_eq!(visible_count, 0);
        Ok(())
    }

    #[tokio::test]
    async fn migration_fresh_install_and_rerun_idempotence() -> Result<(), RepositoryError> {
        let Some(database_url) = std::env::var("DATABASE_URL").ok() else {
            return Ok(());
        };
        let Some((base_url, _database)) = database_url.rsplit_once('/') else {
            return Ok(());
        };
        let admin_url = format!("{base_url}/postgres");
        let db_name = format!("trpg_phase_1a_{}", Uuid::new_v4().simple());
        let admin = PgPoolOptions::new()
            .max_connections(1)
            .connect(&admin_url)
            .await
            .map_err(map_sqlx)?;
        sqlx::query(AssertSqlSafe(format!(r#"CREATE DATABASE "{db_name}""#)))
            .execute(&admin)
            .await
            .map_err(map_sqlx)?;

        let result = async {
            let test_url = format!("{base_url}/{db_name}");
            let pool = PgPoolOptions::new()
                .max_connections(1)
                .connect(&test_url)
                .await
                .map_err(map_sqlx)?;
            MIGRATOR.run(&pool).await.map_err(map_migrate)?;
            MIGRATOR.run(&pool).await.map_err(map_migrate)?;
            pool.close().await;
            Ok::<(), RepositoryError>(())
        }
        .await;

        sqlx::query("SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = $1")
            .bind(&db_name)
            .execute(&admin)
            .await
            .map_err(map_sqlx)?;
        sqlx::query(AssertSqlSafe(format!(r#"DROP DATABASE "{db_name}""#)))
            .execute(&admin)
            .await
            .map_err(map_sqlx)?;
        admin.close().await;

        result
    }
}
