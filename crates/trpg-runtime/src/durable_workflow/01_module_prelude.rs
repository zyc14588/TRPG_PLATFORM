use std::fmt;
use std::str::FromStr;

use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions, PgSslMode};
use sqlx::{PgPool, Postgres, Row, Transaction};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkflowState {
    Pending,
    Running,
    Waiting,
    Completed,
    Failed,
    Cancelled,
    Requested,
    Claimed,
    AgentRunning,
    AwaitingTool,
    Committing,
    RetryableFailed,
    TerminalFailed,
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
            Self::Requested => "REQUESTED",
            Self::Claimed => "CLAIMED",
            Self::AgentRunning => "AGENT_RUNNING",
            Self::AwaitingTool => "AWAITING_TOOL",
            Self::Committing => "COMMITTING",
            Self::RetryableFailed => "RETRYABLE_FAILED",
            Self::TerminalFailed => "TERMINAL_FAILED",
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
            "REQUESTED" => Ok(Self::Requested),
            "CLAIMED" => Ok(Self::Claimed),
            "AGENT_RUNNING" => Ok(Self::AgentRunning),
            "AWAITING_TOOL" => Ok(Self::AwaitingTool),
            "COMMITTING" => Ok(Self::Committing),
            "RETRYABLE_FAILED" => Ok(Self::RetryableFailed),
            "TERMINAL_FAILED" => Ok(Self::TerminalFailed),
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
                | (Self::Requested, Self::Claimed | Self::TerminalFailed)
                | (
                    Self::Claimed,
                    Self::AgentRunning
                        | Self::AwaitingTool
                        | Self::Committing
                        | Self::RetryableFailed
                        | Self::TerminalFailed
                )
                | (
                    Self::AgentRunning,
                    Self::Claimed
                        | Self::AwaitingTool
                        | Self::RetryableFailed
                        | Self::TerminalFailed
                )
                | (
                    Self::AwaitingTool,
                    Self::Claimed
                        | Self::Committing
                        | Self::RetryableFailed
                        | Self::TerminalFailed
                )
                | (
                    Self::Committing,
                    Self::Claimed
                        | Self::Completed
                        | Self::RetryableFailed
                        | Self::TerminalFailed
                )
                | (Self::RetryableFailed, Self::Claimed | Self::TerminalFailed)
        )
    }

    fn releases_lease(self) -> bool {
        matches!(
            self,
            Self::Waiting
                | Self::Completed
                | Self::Failed
                | Self::Cancelled
                | Self::RetryableFailed
                | Self::TerminalFailed
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
