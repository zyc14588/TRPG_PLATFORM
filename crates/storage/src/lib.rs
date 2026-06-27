use async_trait::async_trait;
use auth::{
    AcceptRoomInviteIdempotentCommand, AcceptRoomInviteResult, AuditLog, AuditLogRepository,
    AuthContext, CreateRoomIdempotentCommand, CreateRoomInviteIdempotentCommand, EmailAddress,
    IdempotencyCheck, IdempotencyRecord, IdempotencyRepository, IdempotencyStatus,
    IdempotentOutcome, IdentityRepository, MagicLinkChallenge, RefreshRotationOutcome,
    RefreshSession, RefreshSessionId, RefreshSessionRepository, RefreshSessionStatus,
    RepositoryError, RepositoryTransaction, Room, RoomCommandRepository, RoomId, RoomInvite,
    RoomInviteStatus, RoomMember, RoomPrivacyMode, RoomRepository, RoomRole, RoomWithRole,
    TokenHash, TransactionalRepository, User, UserId,
};
use serde_json::Value;
use sqlx::{postgres::PgPoolOptions, PgPool, Postgres, Row, Transaction};
use std::{
    str::FromStr,
    time::{Duration, SystemTime},
};
use uuid::Uuid;

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations");

#[derive(Debug, Clone)]
pub struct PostgresRepositories {
    pool: PgPool,
    rls_role: Option<String>,
    private_role: Option<String>,
}

impl PostgresRepositories {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            rls_role: None,
            private_role: None,
        }
    }

    pub fn with_rls_role(mut self, role: impl Into<String>) -> Result<Self, RepositoryError> {
        let role = role.into();
        validate_role_name(&role)?;
        self.rls_role = Some(role);
        Ok(self)
    }

    pub fn with_private_role(mut self, role: impl Into<String>) -> Result<Self, RepositoryError> {
        let role = role.into();
        validate_role_name(&role)?;
        self.private_role = Some(role);
        Ok(self)
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

    pub async fn find_user_by_id(&self, user_id: UserId) -> Result<Option<User>, RepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT id, email, display_name
            FROM users
            WHERE id = $1
            "#,
        )
        .bind(user_id.0)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;

        row.map(user_from_row).transpose()
    }

    pub async fn create_magic_link_challenge(
        &self,
        challenge: &MagicLinkChallenge,
    ) -> Result<(), RepositoryError> {
        let mut tx = self.begin_private_tx().await?;
        sqlx::query(
            r#"
            INSERT INTO magic_link_challenges (id, email, token_hash, expires_at)
            VALUES ($1, $2, $3, to_timestamp($4))
            "#,
        )
        .bind(challenge.challenge_id)
        .bind(challenge.email.as_str())
        .bind(challenge.token_hash.as_str())
        .bind(unix_seconds(challenge.expires_at)?)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(())
    }

    pub async fn find_pending_magic_link_challenge_by_token_hash(
        &self,
        token_hash: &TokenHash,
    ) -> Result<Option<MagicLinkChallenge>, RepositoryError> {
        let mut tx = self.begin_private_tx().await?;
        let row = sqlx::query(
            r#"
            SELECT id,
                   email,
                   token_hash,
                   extract(epoch from expires_at)::bigint AS expires_at_epoch
            FROM magic_link_challenges
            WHERE token_hash = $1 AND consumed_at IS NULL
            "#,
        )
        .bind(token_hash.as_str())
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?;

        let challenge = row.map(magic_link_challenge_from_row).transpose()?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(challenge)
    }

    pub async fn consume_magic_link_challenge(
        &self,
        challenge_id: Uuid,
    ) -> Result<bool, RepositoryError> {
        let mut tx = self.begin_private_tx().await?;
        let result = sqlx::query(
            r#"
            UPDATE magic_link_challenges
            SET consumed_at = now()
            WHERE id = $1 AND consumed_at IS NULL
            "#,
        )
        .bind(challenge_id)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn set_rls_context(
        tx: &mut Transaction<'_, Postgres>,
        ctx: &AuthContext,
    ) -> Result<(), RepositoryError> {
        set_rls_values(
            tx,
            Some(ctx.user_id),
            ctx.room_id,
            Some(ctx.role.as_str()),
            None,
        )
        .await
    }

    pub async fn create_room_with_rls(&self, room: &Room) -> Result<(), RepositoryError> {
        let mut tx = self
            .begin_rls_tx(
                Some(room.owner_id.0),
                Some(room.id.0),
                Some(RoomRole::Owner.as_str()),
                None,
            )
            .await?;
        insert_room(&mut tx, room).await?;
        insert_room_member(
            &mut tx,
            &RoomMember {
                room_id: room.id,
                user_id: room.owner_id,
                role: RoomRole::Owner,
            },
        )
        .await?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(())
    }

    pub async fn get_room_with_rls(
        &self,
        room_id: RoomId,
        member: &RoomMember,
    ) -> Result<Option<Room>, RepositoryError> {
        let mut tx = self
            .begin_rls_tx(
                Some(member.user_id.0),
                Some(room_id.0),
                Some(member.role.as_str()),
                None,
            )
            .await?;
        let room = select_room(&mut tx, room_id).await?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(room)
    }

    pub async fn get_room_for_invite_with_rls(
        &self,
        room_id: RoomId,
        email: &EmailAddress,
        user_id: UserId,
    ) -> Result<Option<Room>, RepositoryError> {
        let mut tx = self
            .begin_rls_tx(Some(user_id.0), Some(room_id.0), None, Some(email))
            .await?;
        let room = select_room(&mut tx, room_id).await?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(room)
    }

    pub async fn list_rooms_for_user_with_rls(
        &self,
        user_id: UserId,
    ) -> Result<Vec<RoomWithRole>, RepositoryError> {
        let mut tx = self.begin_rls_tx(Some(user_id.0), None, None, None).await?;
        let rows = sqlx::query(
            r#"
            SELECT r.id,
                   r.owner_id,
                   r.title,
                   r.system_name,
                   r.privacy_mode,
                   r.version,
                   rm.role
            FROM rooms r
            JOIN room_members rm ON rm.room_id = r.id
            WHERE rm.user_id = $1
            ORDER BY r.updated_at DESC, r.created_at DESC
            "#,
        )
        .bind(user_id.0)
        .fetch_all(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let rooms = rows
            .into_iter()
            .map(room_with_role_from_row)
            .collect::<Result<Vec<_>, _>>()?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(rooms)
    }

    pub async fn get_room_member_with_rls(
        &self,
        room_id: RoomId,
        user_id: UserId,
    ) -> Result<Option<RoomMember>, RepositoryError> {
        let mut tx = self
            .begin_rls_tx(
                Some(user_id.0),
                Some(room_id.0),
                Some(RoomRole::Pl.as_str()),
                None,
            )
            .await?;
        let row = sqlx::query(
            r#"
            SELECT room_id, user_id, role
            FROM room_members
            WHERE room_id = $1 AND user_id = $2
            "#,
        )
        .bind(room_id.0)
        .bind(user_id.0)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let member = row.map(room_member_from_row).transpose()?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(member)
    }

    pub async fn list_room_members_with_rls(
        &self,
        room_id: RoomId,
        member: &RoomMember,
    ) -> Result<Vec<RoomMember>, RepositoryError> {
        let mut tx = self
            .begin_rls_tx(
                Some(member.user_id.0),
                Some(room_id.0),
                Some(member.role.as_str()),
                None,
            )
            .await?;
        let rows = sqlx::query(
            r#"
            SELECT room_id, user_id, role
            FROM room_members
            WHERE room_id = $1
            ORDER BY created_at ASC
            "#,
        )
        .bind(room_id.0)
        .fetch_all(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let members = rows
            .into_iter()
            .map(room_member_from_row)
            .collect::<Result<Vec<_>, _>>()?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(members)
    }

    pub async fn create_room_invite_with_rls(
        &self,
        invite: &RoomInvite,
    ) -> Result<(), RepositoryError> {
        let mut tx = self
            .begin_rls_tx(
                Some(invite.invited_by.0),
                Some(invite.room_id.0),
                Some(RoomRole::Owner.as_str()),
                None,
            )
            .await?;
        insert_room_invite(&mut tx, invite).await?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(())
    }

    pub async fn find_pending_room_invite_for_user_with_rls(
        &self,
        token_hash: &TokenHash,
        email: &EmailAddress,
        user_id: UserId,
    ) -> Result<Option<RoomInvite>, RepositoryError> {
        let mut tx = self
            .begin_rls_tx(Some(user_id.0), None, None, Some(email))
            .await?;
        let row = sqlx::query(
            r#"
            SELECT id,
                   room_id,
                   invited_email,
                   invited_role,
                   token_hash,
                   status,
                   invited_by,
                   accepted_by,
                   extract(epoch from expires_at)::bigint AS expires_at_epoch
            FROM room_invites
            WHERE token_hash = $1 AND invited_email = $2 AND status = 'pending'
            "#,
        )
        .bind(token_hash.as_str())
        .bind(email.as_str())
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let invite = row.map(room_invite_from_row).transpose()?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(invite)
    }

    pub async fn accept_room_invite_with_rls(
        &self,
        invite: &RoomInvite,
        member: &RoomMember,
    ) -> Result<(), RepositoryError> {
        let mut tx = self
            .begin_rls_tx(
                Some(member.user_id.0),
                Some(invite.room_id.0),
                Some(member.role.as_str()),
                Some(&invite.invited_email),
            )
            .await?;
        update_room_invite(&mut tx, invite).await?;
        insert_room_member(&mut tx, member).await?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(())
    }

    pub async fn append_audit_log_with_rls(&self, log: &AuditLog) -> Result<(), RepositoryError> {
        let mut tx = self
            .begin_rls_tx(
                log.actor_id.map(|id| id.0),
                log.room_id.map(|id| id.0),
                None,
                None,
            )
            .await?;
        insert_audit_log(&mut tx, log).await?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(())
    }

    async fn begin_rls_tx(
        &self,
        user_id: Option<Uuid>,
        room_id: Option<Uuid>,
        role: Option<&str>,
        email: Option<&EmailAddress>,
    ) -> Result<Transaction<'_, Postgres>, RepositoryError> {
        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;
        if let Some(role) = &self.rls_role {
            set_database_role(&mut tx, role).await?;
        }
        set_rls_values(&mut tx, user_id, room_id, role, email).await?;
        Ok(tx)
    }

    async fn begin_private_tx(&self) -> Result<Transaction<'_, Postgres>, RepositoryError> {
        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;
        if let Some(role) = &self.private_role {
            set_database_role(&mut tx, role).await?;
        }
        Ok(tx)
    }

    async fn set_private_role(
        &self,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<(), RepositoryError> {
        if let Some(role) = &self.private_role {
            set_database_role(tx, role).await?;
        }
        Ok(())
    }

    async fn reset_role_if_configured(
        &self,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<(), RepositoryError> {
        if self.private_role.is_some() || self.rls_role.is_some() {
            reset_database_role(tx).await?;
        }
        Ok(())
    }
}

fn validate_role_name(role: &str) -> Result<(), RepositoryError> {
    if role
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        Ok(())
    } else {
        Err(RepositoryError::Database(
            "invalid postgres role name".to_owned(),
        ))
    }
}

async fn set_database_role(
    tx: &mut Transaction<'_, Postgres>,
    role: &str,
) -> Result<(), RepositoryError> {
    let sql = format!(r#"SET LOCAL ROLE "{}""#, role);
    sqlx::query(sqlx::AssertSqlSafe(sql))
        .execute(&mut **tx)
        .await
        .map_err(map_sqlx)?;
    Ok(())
}

async fn reset_database_role(tx: &mut Transaction<'_, Postgres>) -> Result<(), RepositoryError> {
    sqlx::query("RESET ROLE")
        .execute(&mut **tx)
        .await
        .map_err(map_sqlx)?;
    Ok(())
}

async fn set_rls_values(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Option<Uuid>,
    room_id: Option<Uuid>,
    role: Option<&str>,
    email: Option<&EmailAddress>,
) -> Result<(), RepositoryError> {
    sqlx::query("SELECT set_config('app.user_id', $1, true)")
        .bind(user_id.map(|id| id.to_string()).unwrap_or_default())
        .execute(&mut **tx)
        .await
        .map_err(map_sqlx)?;
    sqlx::query("SELECT set_config('app.room_id', $1, true)")
        .bind(room_id.map(|id| id.to_string()).unwrap_or_default())
        .execute(&mut **tx)
        .await
        .map_err(map_sqlx)?;
    sqlx::query("SELECT set_config('app.room_role', $1, true)")
        .bind(role.unwrap_or_default())
        .execute(&mut **tx)
        .await
        .map_err(map_sqlx)?;
    sqlx::query("SELECT set_config('app.user_email', $1, true)")
        .bind(email.map(EmailAddress::as_str).unwrap_or_default())
        .execute(&mut **tx)
        .await
        .map_err(map_sqlx)?;
    Ok(())
}

async fn insert_room(
    tx: &mut Transaction<'_, Postgres>,
    room: &Room,
) -> Result<(), RepositoryError> {
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
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx)?;
    Ok(())
}

async fn select_room(
    tx: &mut Transaction<'_, Postgres>,
    room_id: RoomId,
) -> Result<Option<Room>, RepositoryError> {
    let row = sqlx::query(
        r#"
        SELECT id, owner_id, title, system_name, privacy_mode, version
        FROM rooms
        WHERE id = $1
        "#,
    )
    .bind(room_id.0)
    .fetch_optional(&mut **tx)
    .await
    .map_err(map_sqlx)?;

    row.map(room_from_row).transpose()
}

async fn insert_room_member(
    tx: &mut Transaction<'_, Postgres>,
    member: &RoomMember,
) -> Result<(), RepositoryError> {
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
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx)?;
    Ok(())
}

async fn insert_room_invite(
    tx: &mut Transaction<'_, Postgres>,
    invite: &RoomInvite,
) -> Result<(), RepositoryError> {
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
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx)?;
    Ok(())
}

async fn update_room_invite(
    tx: &mut Transaction<'_, Postgres>,
    invite: &RoomInvite,
) -> Result<(), RepositoryError> {
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
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx)?;
    Ok(())
}

async fn insert_audit_log(
    tx: &mut Transaction<'_, Postgres>,
    log: &AuditLog,
) -> Result<(), RepositoryError> {
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
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx)?;
    Ok(())
}

enum IdempotencyTxClaim {
    Claimed,
    Replayed(Value),
    Conflict,
}

async fn claim_idempotency_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    scope: &str,
    key: &str,
    request_hash: &str,
    ttl: Duration,
) -> Result<IdempotencyTxClaim, RepositoryError> {
    sqlx::query(
        "DELETE FROM idempotency_keys WHERE scope = $1 AND key = $2 AND expires_at <= now()",
    )
    .bind(scope)
    .bind(key)
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx)?;

    let expires_at = unix_seconds(SystemTime::now() + ttl)?;
    let inserted = sqlx::query_scalar::<_, String>(
        r#"
        INSERT INTO idempotency_keys (scope, key, request_hash, status, response_json, expires_at)
        VALUES ($1, $2, $3, 'in_progress', NULL, to_timestamp($4))
        ON CONFLICT (scope, key) DO NOTHING
        RETURNING key
        "#,
    )
    .bind(scope)
    .bind(key)
    .bind(request_hash)
    .bind(expires_at)
    .fetch_optional(&mut **tx)
    .await
    .map_err(map_sqlx)?;

    if inserted.is_some() {
        return Ok(IdempotencyTxClaim::Claimed);
    }

    let row = sqlx::query(
        r#"
        SELECT scope, key, request_hash, status, response_json
        FROM idempotency_keys
        WHERE scope = $1 AND key = $2
        FOR UPDATE
        "#,
    )
    .bind(scope)
    .bind(key)
    .fetch_one(&mut **tx)
    .await
    .map_err(map_sqlx)?;

    let existing = idempotency_record_from_row(row)?;
    if existing.request_hash != request_hash {
        return Ok(IdempotencyTxClaim::Conflict);
    }
    if existing.status != IdempotencyStatus::Completed {
        return Err(RepositoryError::IdempotencyConflict);
    }
    let response = existing
        .response_json
        .ok_or(RepositoryError::IdempotencyConflict)?;
    Ok(IdempotencyTxClaim::Replayed(response))
}

async fn complete_idempotency_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    scope: &str,
    key: &str,
    response_json: &Value,
) -> Result<(), RepositoryError> {
    sqlx::query(
        r#"
        UPDATE idempotency_keys
        SET status = 'completed',
            response_json = $3
        WHERE scope = $1 AND key = $2
        "#,
    )
    .bind(scope)
    .bind(key)
    .bind(response_json)
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx)?;
    Ok(())
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
        let mut tx = self.begin_private_tx().await?;
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
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(())
    }

    async fn save_refresh_session(&self, session: &RefreshSession) -> Result<(), RepositoryError> {
        let mut tx = self.begin_private_tx().await?;
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
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(())
    }

    async fn find_refresh_session_by_token_hash(
        &self,
        token_hash: &TokenHash,
    ) -> Result<Option<RefreshSession>, RepositoryError> {
        let mut tx = self.begin_private_tx().await?;
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
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?;

        let session = row.map(refresh_session_from_row).transpose()?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(session)
    }

    async fn rotate_refresh_session(
        &self,
        presented_hash: &TokenHash,
        next_hash: TokenHash,
        now: SystemTime,
    ) -> Result<RefreshRotationOutcome, RepositoryError> {
        let mut tx = self.begin_private_tx().await?;
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
            FOR UPDATE
            "#,
        )
        .bind(presented_hash.as_str())
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?;

        let Some(row) = row else {
            tx.commit().await.map_err(map_sqlx)?;
            return Ok(RefreshRotationOutcome::Rejected(
                auth::RefreshSessionError::InvalidToken,
            ));
        };

        let mut session = refresh_session_from_row(row)?;
        match session.rotate(presented_hash, next_hash, now) {
            Ok(()) => {
                sqlx::query(
                    r#"
                    UPDATE refresh_sessions
                    SET current_token_hash = $2,
                        previous_token_hash = $3,
                        status = $4,
                        expires_at = to_timestamp($5),
                        rotated_at = to_timestamp($6),
                        revoked_at = NULL,
                        updated_at = now()
                    WHERE id = $1
                    "#,
                )
                .bind(session.id.0)
                .bind(session.current_token_hash.as_str())
                .bind(session.previous_token_hash.as_ref().map(TokenHash::as_str))
                .bind(session.status.as_str())
                .bind(unix_seconds(session.expires_at)?)
                .bind(optional_unix_seconds(session.rotated_at)?.ok_or_else(|| {
                    RepositoryError::Database(
                        "rotated refresh session missing rotated_at".to_owned(),
                    )
                })?)
                .execute(&mut *tx)
                .await
                .map_err(map_sqlx)?;
                tx.commit().await.map_err(map_sqlx)?;
                Ok(RefreshRotationOutcome::Rotated(session))
            }
            Err(auth::RefreshSessionError::ReuseDetected) => {
                sqlx::query(
                    r#"
                    UPDATE refresh_sessions
                    SET status = 'revoked',
                        revoked_at = to_timestamp($2),
                        updated_at = now()
                    WHERE session_family_id = $1
                    "#,
                )
                .bind(session.session_family_id)
                .bind(unix_seconds(now)?)
                .execute(&mut *tx)
                .await
                .map_err(map_sqlx)?;
                tx.commit().await.map_err(map_sqlx)?;
                Ok(RefreshRotationOutcome::Rejected(
                    auth::RefreshSessionError::ReuseDetected,
                ))
            }
            Err(auth::RefreshSessionError::Expired) => {
                sqlx::query(
                    r#"
                    UPDATE refresh_sessions
                    SET status = 'expired',
                        updated_at = now()
                    WHERE id = $1
                    "#,
                )
                .bind(session.id.0)
                .execute(&mut *tx)
                .await
                .map_err(map_sqlx)?;
                tx.commit().await.map_err(map_sqlx)?;
                Ok(RefreshRotationOutcome::Rejected(
                    auth::RefreshSessionError::Expired,
                ))
            }
            Err(error) => {
                tx.commit().await.map_err(map_sqlx)?;
                Ok(RefreshRotationOutcome::Rejected(error))
            }
        }
    }
}

#[async_trait]
impl RoomRepository for PostgresRepositories {
    async fn create_room(&self, room: &Room) -> Result<(), RepositoryError> {
        self.create_room_with_rls(room).await
    }

    async fn get_room(&self, room_id: RoomId) -> Result<Option<Room>, RepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT id, owner_id, title, system_name, privacy_mode, version
            FROM rooms
            WHERE id = $1
            "#,
        )
        .bind(room_id.0)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;

        row.map(room_from_row).transpose()
    }

    async fn list_rooms_for_user(
        &self,
        user_id: UserId,
    ) -> Result<Vec<RoomWithRole>, RepositoryError> {
        self.list_rooms_for_user_with_rls(user_id).await
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
        self.get_room_member_with_rls(room_id, user_id).await
    }

    async fn list_room_members(&self, room_id: RoomId) -> Result<Vec<RoomMember>, RepositoryError> {
        let rows = sqlx::query(
            r#"
            SELECT room_id, user_id, role
            FROM room_members
            WHERE room_id = $1
            ORDER BY created_at ASC
            "#,
        )
        .bind(room_id.0)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx)?;

        rows.into_iter().map(room_member_from_row).collect()
    }

    async fn create_room_invite(&self, invite: &RoomInvite) -> Result<(), RepositoryError> {
        self.create_room_invite_with_rls(invite).await
    }

    async fn find_pending_room_invite_by_token_hash(
        &self,
        token_hash: &TokenHash,
    ) -> Result<Option<RoomInvite>, RepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT id,
                   room_id,
                   invited_email,
                   invited_role,
                   token_hash,
                   status,
                   invited_by,
                   accepted_by,
                   extract(epoch from expires_at)::bigint AS expires_at_epoch
            FROM room_invites
            WHERE token_hash = $1 AND status = 'pending'
            "#,
        )
        .bind(token_hash.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;

        row.map(room_invite_from_row).transpose()
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

    async fn accept_room_invite(
        &self,
        invite: &RoomInvite,
        member: &RoomMember,
    ) -> Result<(), RepositoryError> {
        self.accept_room_invite_with_rls(invite, member).await
    }
}

#[async_trait]
impl AuditLogRepository for PostgresRepositories {
    async fn append_audit_log(&self, log: &AuditLog) -> Result<(), RepositoryError> {
        self.append_audit_log_with_rls(log).await
    }
}

#[async_trait]
impl RoomCommandRepository for PostgresRepositories {
    async fn create_room_idempotent(
        &self,
        command: CreateRoomIdempotentCommand,
    ) -> Result<IdempotentOutcome<Value>, RepositoryError> {
        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;
        self.set_private_role(&mut tx).await?;
        match claim_idempotency_in_tx(
            &mut tx,
            &command.scope,
            &command.key,
            &command.request_hash,
            command.ttl,
        )
        .await?
        {
            IdempotencyTxClaim::Claimed => {}
            IdempotencyTxClaim::Replayed(response) => {
                tx.commit().await.map_err(map_sqlx)?;
                return Ok(IdempotentOutcome::Replayed(response));
            }
            IdempotencyTxClaim::Conflict => {
                tx.commit().await.map_err(map_sqlx)?;
                return Ok(IdempotentOutcome::Conflict);
            }
        }

        self.reset_role_if_configured(&mut tx).await?;
        if let Some(role) = &self.rls_role {
            set_database_role(&mut tx, role).await?;
        }
        set_rls_values(
            &mut tx,
            Some(command.room.owner_id.0),
            Some(command.room.id.0),
            Some(RoomRole::Owner.as_str()),
            None,
        )
        .await?;
        insert_room(&mut tx, &command.room).await?;
        insert_room_member(
            &mut tx,
            &RoomMember {
                room_id: command.room.id,
                user_id: command.room.owner_id,
                role: RoomRole::Owner,
            },
        )
        .await?;
        insert_audit_log(&mut tx, &command.audit_log).await?;
        self.reset_role_if_configured(&mut tx).await?;
        self.set_private_role(&mut tx).await?;
        complete_idempotency_in_tx(
            &mut tx,
            &command.scope,
            &command.key,
            &command.response_json,
        )
        .await?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(IdempotentOutcome::Created(command.response_json))
    }

    async fn create_room_invite_idempotent(
        &self,
        command: CreateRoomInviteIdempotentCommand,
    ) -> Result<IdempotentOutcome<Value>, RepositoryError> {
        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;
        self.set_private_role(&mut tx).await?;
        match claim_idempotency_in_tx(
            &mut tx,
            &command.scope,
            &command.key,
            &command.request_hash,
            command.ttl,
        )
        .await?
        {
            IdempotencyTxClaim::Claimed => {}
            IdempotencyTxClaim::Replayed(response) => {
                tx.commit().await.map_err(map_sqlx)?;
                return Ok(IdempotentOutcome::Replayed(response));
            }
            IdempotencyTxClaim::Conflict => {
                tx.commit().await.map_err(map_sqlx)?;
                return Ok(IdempotentOutcome::Conflict);
            }
        }

        self.reset_role_if_configured(&mut tx).await?;
        if let Some(role) = &self.rls_role {
            set_database_role(&mut tx, role).await?;
        }
        set_rls_values(
            &mut tx,
            Some(command.invite.invited_by.0),
            Some(command.invite.room_id.0),
            Some(RoomRole::Owner.as_str()),
            None,
        )
        .await?;
        insert_room_invite(&mut tx, &command.invite).await?;
        insert_audit_log(&mut tx, &command.audit_log).await?;
        self.reset_role_if_configured(&mut tx).await?;
        self.set_private_role(&mut tx).await?;
        complete_idempotency_in_tx(
            &mut tx,
            &command.scope,
            &command.key,
            &command.response_json,
        )
        .await?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(IdempotentOutcome::Created(command.response_json))
    }

    async fn accept_room_invite_idempotent(
        &self,
        command: AcceptRoomInviteIdempotentCommand,
    ) -> Result<IdempotentOutcome<AcceptRoomInviteResult>, RepositoryError> {
        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;
        self.set_private_role(&mut tx).await?;
        match claim_idempotency_in_tx(
            &mut tx,
            &command.scope,
            &command.key,
            &command.request_hash,
            command.ttl,
        )
        .await?
        {
            IdempotencyTxClaim::Claimed => {}
            IdempotencyTxClaim::Replayed(response) => {
                tx.commit().await.map_err(map_sqlx)?;
                let replayed = serde_json::from_value(response)
                    .map_err(|err| RepositoryError::Database(err.to_string()))?;
                return Ok(IdempotentOutcome::Replayed(replayed));
            }
            IdempotencyTxClaim::Conflict => {
                tx.commit().await.map_err(map_sqlx)?;
                return Ok(IdempotentOutcome::Conflict);
            }
        }

        self.reset_role_if_configured(&mut tx).await?;
        if let Some(role) = &self.rls_role {
            set_database_role(&mut tx, role).await?;
        }
        set_rls_values(
            &mut tx,
            Some(command.user_id.0),
            None,
            None,
            Some(&command.user_email),
        )
        .await?;

        let row = sqlx::query(
            r#"
            SELECT id,
                   room_id,
                   invited_email,
                   invited_role,
                   token_hash,
                   status,
                   invited_by,
                   accepted_by,
                   extract(epoch from expires_at)::bigint AS expires_at_epoch
            FROM room_invites
            WHERE token_hash = $1 AND invited_email = $2
            FOR UPDATE
            "#,
        )
        .bind(command.token_hash.as_str())
        .bind(command.user_email.as_str())
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let mut invite = row
            .map(room_invite_from_row)
            .transpose()?
            .ok_or(RepositoryError::NotFound)?;
        if invite.status != RoomInviteStatus::Pending {
            return Err(RepositoryError::NotFound);
        }

        let room = select_room(&mut tx, invite.room_id)
            .await?
            .ok_or(RepositoryError::NotFound)?;
        invite
            .accept(command.user_id, command.now)
            .map_err(|err| RepositoryError::Invalid(err.to_string()))?;

        set_rls_values(
            &mut tx,
            Some(command.user_id.0),
            Some(invite.room_id.0),
            Some(invite.role.as_str()),
            Some(&command.user_email),
        )
        .await?;
        update_room_invite(&mut tx, &invite).await?;
        insert_room_member(
            &mut tx,
            &RoomMember {
                room_id: invite.room_id,
                user_id: command.user_id,
                role: invite.role,
            },
        )
        .await?;
        let mut audit_log = command.audit_log;
        audit_log.room_id = Some(invite.room_id);
        audit_log.target_id = Some(invite.id.0);
        audit_log.payload_json = serde_json::json!({ "role": invite.role });
        insert_audit_log(&mut tx, &audit_log).await?;
        self.reset_role_if_configured(&mut tx).await?;
        self.set_private_role(&mut tx).await?;

        let result = AcceptRoomInviteResult {
            room,
            role: invite.role,
        };
        let response_json = serde_json::to_value(&result)
            .map_err(|err| RepositoryError::Database(err.to_string()))?;
        complete_idempotency_in_tx(&mut tx, &command.scope, &command.key, &response_json).await?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(IdempotentOutcome::Created(result))
    }
}

#[async_trait]
impl IdempotencyRepository for PostgresRepositories {
    async fn claim_idempotency_key(
        &self,
        record: &IdempotencyRecord,
        ttl: Duration,
    ) -> Result<IdempotencyCheck, RepositoryError> {
        let mut tx = self.begin_private_tx().await?;
        sqlx::query(
            "DELETE FROM idempotency_keys WHERE scope = $1 AND key = $2 AND expires_at <= now()",
        )
        .bind(&record.scope)
        .bind(&record.key)
        .execute(&mut *tx)
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
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?;

        if inserted.is_some() {
            tx.commit().await.map_err(map_sqlx)?;
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
        .fetch_one(&mut *tx)
        .await
        .map_err(map_sqlx)?;

        let existing = idempotency_record_from_row(row)?;
        let check = if existing.request_hash == record.request_hash {
            Ok(IdempotencyCheck::Duplicate(existing))
        } else {
            Ok(IdempotencyCheck::Conflict)
        }?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(check)
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
            return RepositoryError::Database(format!("{}: {}", code, database_error.message()));
        }
        return RepositoryError::Database(database_error.message().to_owned());
    }
    RepositoryError::Database(error.to_string())
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

fn room_from_row(row: sqlx::postgres::PgRow) -> Result<Room, RepositoryError> {
    let privacy_mode: String = row.try_get("privacy_mode").map_err(map_sqlx)?;
    Ok(Room {
        id: RoomId(row.try_get("id").map_err(map_sqlx)?),
        owner_id: UserId(row.try_get("owner_id").map_err(map_sqlx)?),
        title: row.try_get("title").map_err(map_sqlx)?,
        system_name: row.try_get("system_name").map_err(map_sqlx)?,
        privacy_mode: RoomPrivacyMode::from_str(&privacy_mode)
            .map_err(|err| RepositoryError::Database(err.to_string()))?,
        version: row.try_get("version").map_err(map_sqlx)?,
    })
}

fn room_with_role_from_row(row: sqlx::postgres::PgRow) -> Result<RoomWithRole, RepositoryError> {
    let role: String = row.try_get("role").map_err(map_sqlx)?;
    Ok(RoomWithRole {
        room: room_from_row(row)?,
        role: RoomRole::from_str(&role)
            .map_err(|err| RepositoryError::Database(err.to_string()))?,
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

fn room_invite_from_row(row: sqlx::postgres::PgRow) -> Result<RoomInvite, RepositoryError> {
    let invited_email: String = row.try_get("invited_email").map_err(map_sqlx)?;
    let invited_role: String = row.try_get("invited_role").map_err(map_sqlx)?;
    let token_hash: String = row.try_get("token_hash").map_err(map_sqlx)?;
    let status: String = row.try_get("status").map_err(map_sqlx)?;
    let expires_at_epoch: i64 = row.try_get("expires_at_epoch").map_err(map_sqlx)?;
    Ok(RoomInvite {
        id: auth::RoomInviteId(row.try_get("id").map_err(map_sqlx)?),
        room_id: RoomId(row.try_get("room_id").map_err(map_sqlx)?),
        invited_email: EmailAddress::parse(invited_email)
            .map_err(|err| RepositoryError::Database(err.to_string()))?,
        role: RoomRole::from_str(&invited_role)
            .map_err(|err| RepositoryError::Database(err.to_string()))?,
        token_hash: TokenHash::new(token_hash)
            .map_err(|err| RepositoryError::Database(err.to_string()))?,
        status: room_invite_status_from_str(&status)?,
        invited_by: UserId(row.try_get("invited_by").map_err(map_sqlx)?),
        accepted_by: row
            .try_get::<Option<Uuid>, _>("accepted_by")
            .map_err(map_sqlx)?
            .map(UserId),
        expires_at: from_unix_seconds(expires_at_epoch)?,
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

fn magic_link_challenge_from_row(
    row: sqlx::postgres::PgRow,
) -> Result<MagicLinkChallenge, RepositoryError> {
    let email: String = row.try_get("email").map_err(map_sqlx)?;
    let token_hash: String = row.try_get("token_hash").map_err(map_sqlx)?;
    let expires_at_epoch: i64 = row.try_get("expires_at_epoch").map_err(map_sqlx)?;

    Ok(MagicLinkChallenge {
        challenge_id: row.try_get("id").map_err(map_sqlx)?,
        email: EmailAddress::parse(email)
            .map_err(|err| RepositoryError::Database(err.to_string()))?,
        token_hash: TokenHash::new(token_hash)
            .map_err(|err| RepositoryError::Database(err.to_string()))?,
        expires_at: from_unix_seconds(expires_at_epoch)?,
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

fn room_invite_status_from_str(value: &str) -> Result<RoomInviteStatus, RepositoryError> {
    match value {
        "pending" => Ok(RoomInviteStatus::Pending),
        "accepted" => Ok(RoomInviteStatus::Accepted),
        "revoked" => Ok(RoomInviteStatus::Revoked),
        "expired" => Ok(RoomInviteStatus::Expired),
        _ => Err(RepositoryError::Database(format!(
            "unknown room invite status: {value}"
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

    static MIGRATE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    static RLS_ROLE_SETUP_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

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

    fn unique_token_hash(prefix: &str) -> Result<TokenHash, RepositoryError> {
        token_hash(&format!("{prefix}_{}", Uuid::new_v4()))
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
        let _guard = MIGRATE_LOCK.lock().await;
        if MIGRATOR.run(&pool).await.is_err() {
            return None;
        }
        Some(PostgresRepositories::new(pool))
    }

    async fn ensure_rls_test_role(repo: &PostgresRepositories) -> Result<(), RepositoryError> {
        let _guard = RLS_ROLE_SETUP_LOCK.lock().await;
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

    fn app_role_repo(repo: &PostgresRepositories) -> Result<PostgresRepositories, RepositoryError> {
        repo.clone()
            .with_rls_role("trpg_rls_test")?
            .with_private_role("trpg_app_private")
    }

    async fn insert_source(
        repo: &PostgresRepositories,
        room_id: Option<RoomId>,
        title: &str,
        license_status: &str,
        visibility_scope: VisibilityScope,
        created_by: Option<UserId>,
    ) -> Result<Uuid, RepositoryError> {
        sqlx::query_scalar(
            r#"
            INSERT INTO document_sources (
                room_id, source_kind, title, license_status, visibility_scope,
                content_hash, created_by
            )
            VALUES ($1, 'user_upload', $2, $3, $4, $5, $6)
            RETURNING id
            "#,
        )
        .bind(room_id.map(|id| id.0))
        .bind(title)
        .bind(license_status)
        .bind(visibility_scope.as_str())
        .bind(format!("sha256:{}", Uuid::new_v4().simple()))
        .bind(created_by.map(|id| id.0))
        .fetch_one(repo.pool())
        .await
        .map_err(map_sqlx)
    }

    async fn insert_document(
        repo: &PostgresRepositories,
        room_id: Option<RoomId>,
        source_id: Option<Uuid>,
        title: &str,
        license_status: &str,
        visibility_scope: VisibilityScope,
        uploaded_by: Option<UserId>,
    ) -> Result<Uuid, RepositoryError> {
        sqlx::query_scalar(
            r#"
            INSERT INTO documents (
                room_id, source_id, document_type, title, status, visibility_scope,
                license_status, content_hash, uploaded_by
            )
            VALUES ($1, $2, 'rulebook', $3, $4, $5, $4, $6, $7)
            RETURNING id
            "#,
        )
        .bind(room_id.map(|id| id.0))
        .bind(source_id)
        .bind(title)
        .bind(license_status)
        .bind(visibility_scope.as_str())
        .bind(format!("sha256:{}", Uuid::new_v4().simple()))
        .bind(uploaded_by.map(|id| id.0))
        .fetch_one(repo.pool())
        .await
        .map_err(map_sqlx)
    }

    async fn insert_chunk(
        repo: &PostgresRepositories,
        document_id: Uuid,
        room_id: Option<RoomId>,
        source_id: Option<Uuid>,
        content: &str,
        license_status: &str,
        visibility_scope: VisibilityScope,
    ) -> Result<Uuid, RepositoryError> {
        sqlx::query_scalar(
            r#"
            INSERT INTO chunks (
                document_id, room_id, source_id, visibility_scope, content,
                content_hash, license_status
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id
            "#,
        )
        .bind(document_id)
        .bind(room_id.map(|id| id.0))
        .bind(source_id)
        .bind(visibility_scope.as_str())
        .bind(content)
        .bind(format!("sha256:{}", Uuid::new_v4().simple()))
        .bind(license_status)
        .fetch_one(repo.pool())
        .await
        .map_err(map_sqlx)
    }

    #[tokio::test]
    async fn app_role_can_complete_magic_link_flow() -> Result<(), RepositoryError> {
        let Some(repo) = migrated_repo().await else {
            return Ok(());
        };
        ensure_rls_test_role(&repo).await?;
        let app_repo = app_role_repo(&repo)?;
        let challenge = MagicLinkChallenge {
            challenge_id: Uuid::new_v4(),
            email: email("magic@example.test")?,
            token_hash: unique_token_hash("magic_hash")?,
            expires_at: SystemTime::UNIX_EPOCH + Duration::from_secs(600),
        };

        app_repo.create_magic_link_challenge(&challenge).await?;
        let loaded = app_repo
            .find_pending_magic_link_challenge_by_token_hash(&challenge.token_hash)
            .await?;
        assert_eq!(loaded, Some(challenge.clone()));
        assert!(
            app_repo
                .consume_magic_link_challenge(challenge.challenge_id)
                .await?
        );
        assert!(app_repo
            .find_pending_magic_link_challenge_by_token_hash(&challenge.token_hash)
            .await?
            .is_none());
        Ok(())
    }

    #[tokio::test]
    async fn app_role_can_rotate_refresh_session() -> Result<(), RepositoryError> {
        let Some(repo) = migrated_repo().await else {
            return Ok(());
        };
        ensure_rls_test_role(&repo).await?;
        let app_repo = app_role_repo(&repo)?;
        let owner = user("Owner")?;
        repo.upsert_user(&owner).await?;
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
        let old_hash = unique_token_hash("private_refresh_old")?;
        let new_hash = unique_token_hash("private_refresh_new")?;
        let session =
            RefreshSession::active(owner.id, old_hash.clone(), now, Duration::from_secs(3_600));

        app_repo.create_refresh_session(&session).await?;
        let rotated = app_repo
            .rotate_refresh_session(&old_hash, new_hash.clone(), now + Duration::from_secs(1))
            .await?;

        assert!(matches!(rotated, RefreshRotationOutcome::Rotated(_)));
        assert!(app_repo
            .find_refresh_session_by_token_hash(&new_hash)
            .await?
            .is_some());
        Ok(())
    }

    #[tokio::test]
    async fn app_role_can_claim_idempotency_inside_allowed_function_or_role(
    ) -> Result<(), RepositoryError> {
        let Some(repo) = migrated_repo().await else {
            return Ok(());
        };
        ensure_rls_test_role(&repo).await?;
        let app_repo = app_role_repo(&repo)?;
        let record = IdempotencyRecord {
            scope: format!("private:{}", Uuid::new_v4()),
            key: "same-key".to_owned(),
            request_hash: "hash-a".to_owned(),
            status: IdempotencyStatus::InProgress,
            response_json: None,
        };

        let first = app_repo
            .claim_idempotency_key(&record, Duration::from_secs(60))
            .await?;
        let second = app_repo
            .claim_idempotency_key(&record, Duration::from_secs(60))
            .await?;

        assert_eq!(first, IdempotencyCheck::Claimed);
        assert!(matches!(second, IdempotencyCheck::Duplicate(_)));
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
    async fn previous_refresh_token_reuse_policy_is_explicit() -> Result<(), RepositoryError> {
        let Some(repo) = migrated_repo().await else {
            return Ok(());
        };
        let owner = user("Owner")?;
        repo.upsert_user(&owner).await?;
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
        let old_hash = unique_token_hash("old_refresh_hash")?;
        let session =
            RefreshSession::active(owner.id, old_hash.clone(), now, Duration::from_secs(3_600));
        repo.create_refresh_session(&session).await?;

        let new_hash = unique_token_hash("new_refresh_hash")?;
        let rotated = repo
            .rotate_refresh_session(&old_hash, new_hash.clone(), now + Duration::from_secs(1))
            .await?;
        assert!(matches!(rotated, RefreshRotationOutcome::Rotated(_)));

        let reuse_result = repo
            .rotate_refresh_session(
                &old_hash,
                unique_token_hash("reused_refresh_hash")?,
                now + Duration::from_secs(2),
            )
            .await?;

        assert_eq!(
            reuse_result,
            RefreshRotationOutcome::Rejected(auth::RefreshSessionError::ReuseDetected)
        );
        let loaded = repo
            .find_refresh_session_by_token_hash(&new_hash)
            .await?
            .ok_or(RepositoryError::NotFound)?;
        assert_eq!(loaded.status, RefreshSessionStatus::Revoked);
        Ok(())
    }

    #[tokio::test]
    async fn refresh_reuse_revokes_session_family() -> Result<(), RepositoryError> {
        let Some(repo) = migrated_repo().await else {
            return Ok(());
        };
        let owner = user("Owner")?;
        repo.upsert_user(&owner).await?;
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(2_000);
        let old_hash = unique_token_hash("family_old_refresh_hash")?;
        let session =
            RefreshSession::active(owner.id, old_hash.clone(), now, Duration::from_secs(3_600));
        repo.create_refresh_session(&session).await?;

        let sibling_hash = unique_token_hash("family_sibling_refresh_hash")?;
        let mut sibling = RefreshSession::active(
            owner.id,
            sibling_hash.clone(),
            now,
            Duration::from_secs(3_600),
        );
        sibling.session_family_id = session.session_family_id;
        repo.create_refresh_session(&sibling).await?;

        repo.rotate_refresh_session(
            &old_hash,
            unique_token_hash("family_new_refresh_hash")?,
            now + Duration::from_secs(1),
        )
        .await?;
        let reuse_result = repo
            .rotate_refresh_session(
                &old_hash,
                unique_token_hash("family_reused_refresh_hash")?,
                now + Duration::from_secs(2),
            )
            .await?;

        assert!(matches!(
            reuse_result,
            RefreshRotationOutcome::Rejected(auth::RefreshSessionError::ReuseDetected)
        ));
        let sibling = repo
            .find_refresh_session_by_token_hash(&sibling_hash)
            .await?
            .ok_or(RepositoryError::NotFound)?;
        assert_eq!(sibling.status, RefreshSessionStatus::Revoked);
        Ok(())
    }

    #[tokio::test]
    async fn stale_refresh_cookie_after_race_is_rejected() -> Result<(), RepositoryError> {
        let Some(repo) = migrated_repo().await else {
            return Ok(());
        };
        let owner = user("Owner")?;
        repo.upsert_user(&owner).await?;
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(3_000);
        let old_hash = unique_token_hash("stale_old_refresh_hash")?;
        let session =
            RefreshSession::active(owner.id, old_hash.clone(), now, Duration::from_secs(3_600));
        repo.create_refresh_session(&session).await?;

        repo.rotate_refresh_session(
            &old_hash,
            unique_token_hash("stale_new_refresh_hash")?,
            now + Duration::from_secs(1),
        )
        .await?;
        let stale = repo
            .rotate_refresh_session(
                &old_hash,
                unique_token_hash("stale_loser_refresh_hash")?,
                now + Duration::from_secs(2),
            )
            .await?;

        assert_eq!(
            stale,
            RefreshRotationOutcome::Rejected(auth::RefreshSessionError::ReuseDetected)
        );
        Ok(())
    }

    #[tokio::test]
    async fn concurrent_refresh_only_one_rotation_wins() -> Result<(), RepositoryError> {
        let Some(repo) = migrated_repo().await else {
            return Ok(());
        };
        let owner = user("Owner")?;
        repo.upsert_user(&owner).await?;
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(4_000);
        let old_hash = unique_token_hash("concurrent_old_refresh_hash")?;
        let session =
            RefreshSession::active(owner.id, old_hash.clone(), now, Duration::from_secs(3_600));
        repo.create_refresh_session(&session).await?;

        let first_repo = repo.clone();
        let second_repo = repo.clone();
        let first_hash = old_hash.clone();
        let second_hash = old_hash.clone();
        let first = async move {
            first_repo
                .rotate_refresh_session(
                    &first_hash,
                    unique_token_hash("concurrent_first_refresh_hash")?,
                    now + Duration::from_secs(1),
                )
                .await
        };
        let second = async move {
            second_repo
                .rotate_refresh_session(
                    &second_hash,
                    unique_token_hash("concurrent_second_refresh_hash")?,
                    now + Duration::from_secs(1),
                )
                .await
        };
        let (first, second) = tokio::join!(first, second);
        let first = first?;
        let second = second?;
        let results = [&first, &second];

        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(result, RefreshRotationOutcome::Rotated(_)))
                .count(),
            1
        );
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(result, RefreshRotationOutcome::Rejected(_)))
                .count(),
            1
        );
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
        let owner_email = email(&format!("owner-{}@example.test", Uuid::new_v4()))?;
        let owner_challenge = auth_provider
            .issue_magic_link(
                MagicLinkRequest {
                    email: owner_email,
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
            unique_token_hash("vertical_refresh_hash")?,
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
            token_hash: unique_token_hash("vertical_invite_hash")?,
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
                    key: format!("vertical-flow-{}", Uuid::new_v4()),
                    request_hash: format!("vertical-hash-{}", Uuid::new_v4()),
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
    async fn app_role_cannot_select_cross_room_documents() -> Result<(), RepositoryError> {
        let Some(repo) = migrated_repo().await else {
            return Ok(());
        };
        ensure_rls_test_role(&repo).await?;
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
        let source_b = insert_source(
            &repo,
            Some(room_b.id),
            "room b source",
            "allowed",
            VisibilityScope::RoomRule,
            Some(owner.id),
        )
        .await?;
        let doc_b = insert_document(
            &repo,
            Some(room_b.id),
            Some(source_b),
            "room b document",
            "allowed",
            VisibilityScope::RoomRule,
            Some(owner.id),
        )
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
                room_id: Some(room_a.id.0),
                role: RoomRole::Pl,
            },
        )
        .await?;
        let visible_count: i64 = sqlx::query_scalar("SELECT count(*) FROM documents WHERE id = $1")
            .bind(doc_b)
            .fetch_one(&mut *tx)
            .await
            .map_err(map_sqlx)?;
        tx.rollback().await.map_err(map_sqlx)?;

        assert_eq!(visible_count, 0);
        Ok(())
    }

    #[tokio::test]
    async fn app_role_cannot_select_kp_only_as_pl() -> Result<(), RepositoryError> {
        let Some(repo) = migrated_repo().await else {
            return Ok(());
        };
        ensure_rls_test_role(&repo).await?;
        let owner = user("Owner")?;
        let pl = user("PL")?;
        repo.upsert_user(&owner).await?;
        repo.upsert_user(&pl).await?;
        let room = room(owner.id);
        repo.create_room(&room).await?;
        repo.add_room_member(&RoomMember {
            room_id: room.id,
            user_id: pl.id,
            role: RoomRole::Pl,
        })
        .await?;
        let source = insert_source(
            &repo,
            Some(room.id),
            "secret source",
            "allowed",
            VisibilityScope::KpOnlyModule,
            Some(owner.id),
        )
        .await?;
        let doc = insert_document(
            &repo,
            Some(room.id),
            Some(source),
            "secret document",
            "allowed",
            VisibilityScope::KpOnlyModule,
            Some(owner.id),
        )
        .await?;
        let chunk = insert_chunk(
            &repo,
            doc,
            Some(room.id),
            Some(source),
            "kp only",
            "allowed",
            VisibilityScope::KpOnlyModule,
        )
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
        let visible_count: i64 = sqlx::query_scalar("SELECT count(*) FROM chunks WHERE id = $1")
            .bind(chunk)
            .fetch_one(&mut *tx)
            .await
            .map_err(map_sqlx)?;
        tx.rollback().await.map_err(map_sqlx)?;

        assert_eq!(visible_count, 0);
        Ok(())
    }

    #[tokio::test]
    async fn pl_retrieval_cannot_select_pending_or_denied_chunks() -> Result<(), RepositoryError> {
        let Some(repo) = migrated_repo().await else {
            return Ok(());
        };
        ensure_rls_test_role(&repo).await?;
        let owner = user("Owner")?;
        let pl = user("PL")?;
        repo.upsert_user(&owner).await?;
        repo.upsert_user(&pl).await?;
        let room = room(owner.id);
        repo.create_room(&room).await?;
        repo.add_room_member(&RoomMember {
            room_id: room.id,
            user_id: pl.id,
            role: RoomRole::Pl,
        })
        .await?;

        for status in ["allowed", "pending_review", "denied"] {
            let source = insert_source(
                &repo,
                Some(room.id),
                &format!("{status} source"),
                status,
                VisibilityScope::RoomRule,
                Some(owner.id),
            )
            .await?;
            let doc = insert_document(
                &repo,
                Some(room.id),
                Some(source),
                &format!("{status} document"),
                status,
                VisibilityScope::RoomRule,
                Some(owner.id),
            )
            .await?;
            insert_chunk(
                &repo,
                doc,
                Some(room.id),
                Some(source),
                status,
                status,
                VisibilityScope::RoomRule,
            )
            .await?;
        }

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
        let rows: Vec<String> =
            sqlx::query_scalar("SELECT content FROM chunks WHERE room_id = $1 ORDER BY content")
                .bind(room.id.0)
                .fetch_all(&mut *tx)
                .await
                .map_err(map_sqlx)?;
        tx.rollback().await.map_err(map_sqlx)?;

        assert_eq!(rows, vec!["allowed".to_owned()]);
        Ok(())
    }

    #[tokio::test]
    async fn kp_retrieval_cannot_select_denied_chunks() -> Result<(), RepositoryError> {
        let Some(repo) = migrated_repo().await else {
            return Ok(());
        };
        ensure_rls_test_role(&repo).await?;
        let owner = user("Owner")?;
        repo.upsert_user(&owner).await?;
        let room = room(owner.id);
        repo.create_room(&room).await?;

        for status in ["allowed", "denied"] {
            let source = insert_source(
                &repo,
                Some(room.id),
                &format!("{status} kp source"),
                status,
                VisibilityScope::KpOnlyModule,
                Some(owner.id),
            )
            .await?;
            let doc = insert_document(
                &repo,
                Some(room.id),
                Some(source),
                &format!("{status} kp document"),
                status,
                VisibilityScope::KpOnlyModule,
                Some(owner.id),
            )
            .await?;
            insert_chunk(
                &repo,
                doc,
                Some(room.id),
                Some(source),
                status,
                status,
                VisibilityScope::KpOnlyModule,
            )
            .await?;
        }

        let mut tx = repo.pool().begin().await.map_err(map_sqlx)?;
        sqlx::query("SET LOCAL ROLE trpg_rls_test")
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx)?;
        PostgresRepositories::set_rls_context(
            &mut tx,
            &AuthContext {
                user_id: owner.id.0,
                room_id: Some(room.id.0),
                role: RoomRole::Owner,
            },
        )
        .await?;
        let rows: Vec<String> =
            sqlx::query_scalar("SELECT content FROM chunks WHERE room_id = $1 ORDER BY content")
                .bind(room.id.0)
                .fetch_all(&mut *tx)
                .await
                .map_err(map_sqlx)?;
        tx.rollback().await.map_err(map_sqlx)?;

        assert_eq!(rows, vec!["allowed".to_owned()]);
        Ok(())
    }

    #[tokio::test]
    async fn kp_review_can_list_pending_sources_only_through_review_path(
    ) -> Result<(), RepositoryError> {
        let Some(repo) = migrated_repo().await else {
            return Ok(());
        };
        ensure_rls_test_role(&repo).await?;
        let owner = user("Owner")?;
        repo.upsert_user(&owner).await?;
        let room = room(owner.id);
        repo.create_room(&room).await?;
        insert_source(
            &repo,
            Some(room.id),
            "pending source",
            "pending_review",
            VisibilityScope::RoomRule,
            Some(owner.id),
        )
        .await?;

        let mut tx = repo.pool().begin().await.map_err(map_sqlx)?;
        sqlx::query("SET LOCAL ROLE trpg_rls_test")
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx)?;
        PostgresRepositories::set_rls_context(
            &mut tx,
            &AuthContext {
                user_id: owner.id.0,
                room_id: Some(room.id.0),
                role: RoomRole::Owner,
            },
        )
        .await?;
        let ordinary_count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM document_sources WHERE room_id = $1")
                .bind(room.id.0)
                .fetch_one(&mut *tx)
                .await
                .map_err(map_sqlx)?;
        sqlx::query("SELECT set_config('app.rag_access_path', 'license_review', true)")
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx)?;
        let review_count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM document_sources WHERE room_id = $1")
                .bind(room.id.0)
                .fetch_one(&mut *tx)
                .await
                .map_err(map_sqlx)?;
        tx.rollback().await.map_err(map_sqlx)?;

        assert_eq!(ordinary_count, 0);
        assert_eq!(review_count, 1);
        Ok(())
    }

    #[tokio::test]
    async fn public_rule_requires_allowed_license() -> Result<(), RepositoryError> {
        let Some(repo) = migrated_repo().await else {
            return Ok(());
        };
        ensure_rls_test_role(&repo).await?;
        let marker = format!("public-{}", Uuid::new_v4());

        for status in ["allowed", "pending_review", "denied"] {
            let source = insert_source(
                &repo,
                None,
                &format!("{marker} {status} source"),
                status,
                VisibilityScope::PublicRule,
                None,
            )
            .await?;
            let doc = insert_document(
                &repo,
                None,
                Some(source),
                &format!("{marker} {status} document"),
                status,
                VisibilityScope::PublicRule,
                None,
            )
            .await?;
            insert_chunk(
                &repo,
                doc,
                None,
                Some(source),
                &format!("{marker} {status} chunk"),
                status,
                VisibilityScope::PublicRule,
            )
            .await?;
        }

        let mut tx = repo.pool().begin().await.map_err(map_sqlx)?;
        sqlx::query("SET LOCAL ROLE trpg_rls_test")
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx)?;
        let title_like = format!("{marker}%");
        let sources: i64 =
            sqlx::query_scalar("SELECT count(*) FROM document_sources WHERE title LIKE $1")
                .bind(&title_like)
                .fetch_one(&mut *tx)
                .await
                .map_err(map_sqlx)?;
        let documents: i64 =
            sqlx::query_scalar("SELECT count(*) FROM documents WHERE title LIKE $1")
                .bind(&title_like)
                .fetch_one(&mut *tx)
                .await
                .map_err(map_sqlx)?;
        let chunks: i64 = sqlx::query_scalar("SELECT count(*) FROM chunks WHERE content LIKE $1")
            .bind(&title_like)
            .fetch_one(&mut *tx)
            .await
            .map_err(map_sqlx)?;
        tx.rollback().await.map_err(map_sqlx)?;

        assert_eq!((sources, documents, chunks), (1, 1, 1));
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
