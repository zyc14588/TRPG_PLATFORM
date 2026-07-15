use std::fmt;
use std::str::FromStr;

use serde_json::Value;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions, PgSslMode};
use sqlx::{PgPool, Postgres, Row, Transaction};

const BASE_MIGRATION: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../migrations/20260705000100_create_data_eventing_event_store.up.sql"
));
const DURABLE_WORKFLOW_MIGRATION: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../migrations/20260715000400_create_canonical_commit_protocol.up.sql"
));

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkflowState {
    Pending,
    Running,
    Waiting,
    Completed,
    Failed,
    Cancelled,
}

impl WorkflowState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "PENDING",
            Self::Running => "RUNNING",
            Self::Waiting => "WAITING",
            Self::Completed => "COMPLETED",
            Self::Failed => "FAILED",
            Self::Cancelled => "CANCELLED",
        }
    }

    fn parse(value: &str) -> Result<Self, WorkflowStoreError> {
        match value {
            "PENDING" => Ok(Self::Pending),
            "RUNNING" => Ok(Self::Running),
            "WAITING" => Ok(Self::Waiting),
            "COMPLETED" => Ok(Self::Completed),
            "FAILED" => Ok(Self::Failed),
            "CANCELLED" => Ok(Self::Cancelled),
            _ => Err(WorkflowStoreError::IntegrityViolation(
                "unknown_workflow_state",
            )),
        }
    }

    fn can_transition_to(self, target: Self) -> bool {
        matches!(
            (self, target),
            (Self::Pending, Self::Running | Self::Cancelled)
                | (
                    Self::Running,
                    Self::Waiting | Self::Completed | Self::Failed | Self::Cancelled
                )
                | (
                    Self::Waiting,
                    Self::Running | Self::Failed | Self::Cancelled
                )
        )
    }

    fn releases_lease(self) -> bool {
        matches!(
            self,
            Self::Waiting | Self::Completed | Self::Failed | Self::Cancelled
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DurableWorkflow {
    pub workflow_id: String,
    pub campaign_id: String,
    pub workflow_type: String,
    pub state: WorkflowState,
    pub version: i64,
    pub input_json: String,
    pub output_json: Option<String>,
    pub wake_at_unix_ms: Option<i64>,
    pub lease_owner: Option<String>,
    pub lease_expires_at_unix_ms: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowTransitionDraft {
    pub workflow_id: String,
    pub expected_version: i64,
    pub from_state: WorkflowState,
    pub to_state: WorkflowState,
    pub idempotency_key: String,
    pub correlation_id: String,
    pub causation_id: String,
    pub output_json: Option<String>,
    pub wake_at_unix_ms: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowTransition {
    pub transition_id: i64,
    pub workflow_id: String,
    pub from_state: WorkflowState,
    pub to_state: WorkflowState,
    pub workflow_version: i64,
    pub idempotency_key: String,
    pub correlation_id: String,
    pub causation_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkflowStoreError {
    Configuration(&'static str),
    Connection,
    Migration,
    Validation(&'static str),
    NotFound,
    VersionConflict { expected: i64, actual: i64 },
    StateConflict,
    IdempotencyConflict,
    Database(&'static str),
    IntegrityViolation(&'static str),
}

impl fmt::Display for WorkflowStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration(reason) => write!(formatter, "configuration error: {reason}"),
            Self::Connection => formatter.write_str("workflow database connection failed"),
            Self::Migration => formatter.write_str("workflow database migration failed"),
            Self::Validation(reason) => write!(formatter, "workflow validation failed: {reason}"),
            Self::NotFound => formatter.write_str("workflow not found"),
            Self::VersionConflict { expected, actual } => {
                write!(
                    formatter,
                    "expected version {expected}, actual version {actual}"
                )
            }
            Self::StateConflict => formatter.write_str("workflow state conflict"),
            Self::IdempotencyConflict => formatter.write_str("workflow idempotency conflict"),
            Self::Database(operation) => write!(formatter, "workflow database failed: {operation}"),
            Self::IntegrityViolation(reason) => {
                write!(formatter, "workflow integrity violation: {reason}")
            }
        }
    }
}

impl std::error::Error for WorkflowStoreError {}

#[derive(Clone)]
pub struct DurableWorkflowStore {
    pool: PgPool,
}

impl fmt::Debug for DurableWorkflowStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DurableWorkflowStore")
            .field("pool", &"[POSTGRESQL POOL]")
            .finish()
    }
}

impl DurableWorkflowStore {
    pub async fn connect(database_url: &str) -> Result<Self, WorkflowStoreError> {
        let options = PgConnectOptions::from_str(database_url)
            .map_err(|_| WorkflowStoreError::Configuration("invalid_postgresql_url"))?;
        let host = options.get_host();
        let local = matches!(host, "localhost" | "127.0.0.1" | "::1") || host.starts_with('/');
        if !local && !matches!(options.get_ssl_mode(), PgSslMode::VerifyFull) {
            return Err(WorkflowStoreError::Configuration(
                "remote_postgresql_requires_sslmode_verify_full",
            ));
        }
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect_with(options)
            .await
            .map_err(|_| WorkflowStoreError::Connection)?;
        Ok(Self { pool })
    }

    pub async fn apply_migration(&self) -> Result<(), WorkflowStoreError> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| WorkflowStoreError::Migration)?;
        sqlx::raw_sql(BASE_MIGRATION)
            .execute(&mut *transaction)
            .await
            .map_err(|_| WorkflowStoreError::Migration)?;
        sqlx::raw_sql(DURABLE_WORKFLOW_MIGRATION)
            .execute(&mut *transaction)
            .await
            .map_err(|_| WorkflowStoreError::Migration)?;
        transaction
            .commit()
            .await
            .map_err(|_| WorkflowStoreError::Migration)
    }

    pub async fn check_readiness(&self) -> Result<(), WorkflowStoreError> {
        let exists: bool = sqlx::query_scalar(
            "SELECT to_regclass('workflow_instances') IS NOT NULL \
             AND to_regclass('workflow_transitions') IS NOT NULL",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|_| WorkflowStoreError::Database("readiness"))?;
        if exists {
            Ok(())
        } else {
            Err(WorkflowStoreError::Migration)
        }
    }

    pub async fn create(
        &self,
        workflow_id: &str,
        campaign_id: &str,
        workflow_type: &str,
        input_json: &str,
        wake_at_unix_ms: Option<i64>,
    ) -> Result<DurableWorkflow, WorkflowStoreError> {
        validate_identifier(workflow_id, "workflow_id_required")?;
        validate_identifier(campaign_id, "campaign_id_required")?;
        validate_identifier(workflow_type, "workflow_type_required")?;
        validate_timestamp(wake_at_unix_ms)?;
        let input_json = normalize_json(input_json)?;

        let inserted = sqlx::query(
            r#"
            INSERT INTO workflow_instances (
                workflow_id, campaign_id, workflow_type, state, version,
                input_json, wake_at
            ) VALUES ($1, $2, $3, 'PENDING', 0, $4,
                CASE WHEN $5::bigint IS NULL THEN NULL
                     ELSE to_timestamp($5::bigint / 1000.0) END)
            ON CONFLICT (workflow_id) DO NOTHING
            RETURNING workflow_id
            "#,
        )
        .bind(workflow_id)
        .bind(campaign_id)
        .bind(workflow_type)
        .bind(&input_json)
        .bind(wake_at_unix_ms)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| WorkflowStoreError::Database("create"))?;

        let workflow = self
            .load(workflow_id)
            .await?
            .ok_or(WorkflowStoreError::NotFound)?;
        if inserted.is_none()
            && (workflow.campaign_id != campaign_id
                || workflow.workflow_type != workflow_type
                || workflow.input_json != input_json
                || workflow.wake_at_unix_ms != wake_at_unix_ms)
        {
            return Err(WorkflowStoreError::IdempotencyConflict);
        }
        Ok(workflow)
    }

    pub async fn load(
        &self,
        workflow_id: &str,
    ) -> Result<Option<DurableWorkflow>, WorkflowStoreError> {
        let row = sqlx::query(
            r#"
            SELECT workflow_id, campaign_id, workflow_type, state, version,
                   input_json, output_json,
                   CASE WHEN wake_at IS NULL THEN NULL
                        ELSE (extract(epoch FROM wake_at) * 1000)::bigint END AS wake_at_unix_ms,
                   lease_owner,
                   CASE WHEN lease_expires_at IS NULL THEN NULL
                        ELSE (extract(epoch FROM lease_expires_at) * 1000)::bigint END
                        AS lease_expires_at_unix_ms
              FROM workflow_instances WHERE workflow_id = $1
            "#,
        )
        .bind(workflow_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| WorkflowStoreError::Database("load"))?;
        row.map(|row| workflow_from_row(&row)).transpose()
    }

    pub async fn transition(
        &self,
        draft: &WorkflowTransitionDraft,
    ) -> Result<WorkflowTransition, WorkflowStoreError> {
        validate_transition(draft)?;
        let output_json = draft
            .output_json
            .as_deref()
            .map(normalize_json)
            .transpose()?;
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| WorkflowStoreError::Database("begin_transition"))?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(&draft.workflow_id)
            .execute(&mut *transaction)
            .await
            .map_err(|_| WorkflowStoreError::Database("lock_workflow"))?;

        if let Some(existing) =
            load_transition_by_idempotency(&mut transaction, &draft.idempotency_key).await?
        {
            if existing.workflow_id != draft.workflow_id
                || existing.from_state != draft.from_state
                || existing.to_state != draft.to_state
                || existing.workflow_version != draft.expected_version + 1
                || existing.correlation_id != draft.correlation_id
                || existing.causation_id != draft.causation_id
            {
                return Err(WorkflowStoreError::IdempotencyConflict);
            }
            transaction
                .commit()
                .await
                .map_err(|_| WorkflowStoreError::Database("commit_idempotent_transition"))?;
            return Ok(existing);
        }

        let current = sqlx::query(
            "SELECT state, version FROM workflow_instances WHERE workflow_id = $1 FOR UPDATE",
        )
        .bind(&draft.workflow_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| WorkflowStoreError::Database("load_transition_state"))?
        .ok_or(WorkflowStoreError::NotFound)?;
        let current_state = WorkflowState::parse(current.get::<String, _>("state").as_str())?;
        let current_version: i64 = current.get("version");
        if current_version != draft.expected_version {
            return Err(WorkflowStoreError::VersionConflict {
                expected: draft.expected_version,
                actual: current_version,
            });
        }
        if current_state != draft.from_state || !current_state.can_transition_to(draft.to_state) {
            return Err(WorkflowStoreError::StateConflict);
        }
        let workflow_version = current_version + 1;
        let transition_id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO workflow_transitions (
                workflow_id, from_state, to_state, workflow_version,
                idempotency_key, correlation_id, causation_id
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING transition_id
            "#,
        )
        .bind(&draft.workflow_id)
        .bind(draft.from_state.as_str())
        .bind(draft.to_state.as_str())
        .bind(workflow_version)
        .bind(&draft.idempotency_key)
        .bind(&draft.correlation_id)
        .bind(&draft.causation_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|_| WorkflowStoreError::Database("insert_transition"))?;

        let updated = sqlx::query(
            r#"
            UPDATE workflow_instances
               SET state = $2,
                   version = $3,
                   output_json = COALESCE($4, output_json),
                   wake_at = CASE WHEN $5::bigint IS NULL THEN NULL
                                  ELSE to_timestamp($5::bigint / 1000.0) END,
                   lease_owner = CASE WHEN $6 THEN NULL ELSE lease_owner END,
                   lease_expires_at = CASE WHEN $6 THEN NULL ELSE lease_expires_at END,
                   updated_at = now()
             WHERE workflow_id = $1 AND version = $7 AND state = $8
            "#,
        )
        .bind(&draft.workflow_id)
        .bind(draft.to_state.as_str())
        .bind(workflow_version)
        .bind(output_json)
        .bind(draft.wake_at_unix_ms)
        .bind(draft.to_state.releases_lease())
        .bind(draft.expected_version)
        .bind(draft.from_state.as_str())
        .execute(&mut *transaction)
        .await
        .map_err(|_| WorkflowStoreError::Database("update_workflow"))?;
        if updated.rows_affected() != 1 {
            return Err(WorkflowStoreError::StateConflict);
        }
        transaction
            .commit()
            .await
            .map_err(|_| WorkflowStoreError::Database("commit_transition"))?;

        Ok(WorkflowTransition {
            transition_id,
            workflow_id: draft.workflow_id.clone(),
            from_state: draft.from_state,
            to_state: draft.to_state,
            workflow_version,
            idempotency_key: draft.idempotency_key.clone(),
            correlation_id: draft.correlation_id.clone(),
            causation_id: draft.causation_id.clone(),
        })
    }

    pub async fn acquire_due(
        &self,
        lease_owner: &str,
        now_unix_ms: i64,
        lease_duration_ms: i64,
        limit: i64,
    ) -> Result<Vec<DurableWorkflow>, WorkflowStoreError> {
        validate_identifier(lease_owner, "lease_owner_required")?;
        if now_unix_ms < 0 || lease_duration_ms <= 0 || limit <= 0 || limit > 1_000 {
            return Err(WorkflowStoreError::Validation("invalid_lease_request"));
        }
        let rows = sqlx::query(
            r#"
            WITH due AS (
                SELECT workflow_id
                  FROM workflow_instances
                 WHERE state IN ('PENDING', 'WAITING', 'RUNNING')
                   AND (wake_at IS NULL OR wake_at <= to_timestamp($2::bigint / 1000.0))
                   AND (
                       lease_expires_at IS NULL
                       OR lease_expires_at <= to_timestamp($2::bigint / 1000.0)
                   )
                 ORDER BY COALESCE(wake_at, to_timestamp(0)), workflow_id
                 FOR UPDATE SKIP LOCKED
                 LIMIT $4
            )
            UPDATE workflow_instances workflow
               SET lease_owner = $1,
                   lease_expires_at = to_timestamp(($2::bigint + $3::bigint) / 1000.0),
                   updated_at = now()
              FROM due
             WHERE workflow.workflow_id = due.workflow_id
            RETURNING workflow.workflow_id, workflow.campaign_id, workflow.workflow_type,
                      workflow.state, workflow.version, workflow.input_json,
                      workflow.output_json,
                      CASE WHEN workflow.wake_at IS NULL THEN NULL
                           ELSE (extract(epoch FROM workflow.wake_at) * 1000)::bigint END
                           AS wake_at_unix_ms,
                      workflow.lease_owner,
                      (extract(epoch FROM workflow.lease_expires_at) * 1000)::bigint
                           AS lease_expires_at_unix_ms
            "#,
        )
        .bind(lease_owner)
        .bind(now_unix_ms)
        .bind(lease_duration_ms)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| WorkflowStoreError::Database("acquire_due"))?;
        rows.iter().map(workflow_from_row).collect()
    }
}

async fn load_transition_by_idempotency(
    transaction: &mut Transaction<'_, Postgres>,
    idempotency_key: &str,
) -> Result<Option<WorkflowTransition>, WorkflowStoreError> {
    let row = sqlx::query(
        r#"
        SELECT transition_id, workflow_id, from_state, to_state,
               workflow_version, idempotency_key, correlation_id, causation_id
          FROM workflow_transitions WHERE idempotency_key = $1
        "#,
    )
    .bind(idempotency_key)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|_| WorkflowStoreError::Database("load_idempotent_transition"))?;
    row.map(|row| transition_from_row(&row)).transpose()
}

fn workflow_from_row(row: &sqlx::postgres::PgRow) -> Result<DurableWorkflow, WorkflowStoreError> {
    Ok(DurableWorkflow {
        workflow_id: row.get("workflow_id"),
        campaign_id: row.get("campaign_id"),
        workflow_type: row.get("workflow_type"),
        state: WorkflowState::parse(row.get::<String, _>("state").as_str())?,
        version: row.get("version"),
        input_json: row.get("input_json"),
        output_json: row.get("output_json"),
        wake_at_unix_ms: row.get("wake_at_unix_ms"),
        lease_owner: row.get("lease_owner"),
        lease_expires_at_unix_ms: row.get("lease_expires_at_unix_ms"),
    })
}

fn transition_from_row(
    row: &sqlx::postgres::PgRow,
) -> Result<WorkflowTransition, WorkflowStoreError> {
    Ok(WorkflowTransition {
        transition_id: row.get("transition_id"),
        workflow_id: row.get("workflow_id"),
        from_state: WorkflowState::parse(row.get::<String, _>("from_state").as_str())?,
        to_state: WorkflowState::parse(row.get::<String, _>("to_state").as_str())?,
        workflow_version: row.get("workflow_version"),
        idempotency_key: row.get("idempotency_key"),
        correlation_id: row.get("correlation_id"),
        causation_id: row.get("causation_id"),
    })
}

fn validate_transition(draft: &WorkflowTransitionDraft) -> Result<(), WorkflowStoreError> {
    validate_identifier(&draft.workflow_id, "workflow_id_required")?;
    validate_identifier(&draft.idempotency_key, "idempotency_key_required")?;
    validate_identifier(&draft.correlation_id, "correlation_id_required")?;
    validate_identifier(&draft.causation_id, "causation_id_required")?;
    if draft.expected_version < 0 {
        return Err(WorkflowStoreError::Validation(
            "non_negative_expected_version_required",
        ));
    }
    validate_timestamp(draft.wake_at_unix_ms)?;
    if !draft.from_state.can_transition_to(draft.to_state) {
        return Err(WorkflowStoreError::StateConflict);
    }
    Ok(())
}

fn validate_identifier(value: &str, reason: &'static str) -> Result<(), WorkflowStoreError> {
    if value.trim().is_empty() || value.len() > 256 {
        Err(WorkflowStoreError::Validation(reason))
    } else {
        Ok(())
    }
}

fn validate_timestamp(value: Option<i64>) -> Result<(), WorkflowStoreError> {
    if value.is_some_and(|value| value < 0) {
        Err(WorkflowStoreError::Validation(
            "non_negative_wake_time_required",
        ))
    } else {
        Ok(())
    }
}

fn normalize_json(input: &str) -> Result<String, WorkflowStoreError> {
    if input.len() > 1_048_576 {
        return Err(WorkflowStoreError::Validation("workflow_json_too_large"));
    }
    let value: Value =
        serde_json::from_str(input).map_err(|_| WorkflowStoreError::Validation("invalid_json"))?;
    serde_json::to_string(&value).map_err(|_| WorkflowStoreError::Validation("invalid_json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_states_cannot_reenter_the_workflow() {
        assert!(!WorkflowState::Completed.can_transition_to(WorkflowState::Running));
        assert!(!WorkflowState::Failed.can_transition_to(WorkflowState::Running));
        assert!(!WorkflowState::Cancelled.can_transition_to(WorkflowState::Running));
    }

    #[test]
    fn remote_database_requires_hostname_verification() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let error = runtime
            .block_on(DurableWorkflowStore::connect(
                "postgresql://app@example.invalid/trpg?sslmode=require",
            ))
            .unwrap_err();
        assert_eq!(
            error,
            WorkflowStoreError::Configuration("remote_postgresql_requires_sslmode_verify_full")
        );
    }
}
