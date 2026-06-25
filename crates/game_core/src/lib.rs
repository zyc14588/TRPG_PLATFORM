use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExpectedVersion(pub i64);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IdempotencyKey(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GridGeometry {
    SceneBoard,
    Square { cell_size: f32 },
    HexFlatTop { hex_size: f32 },
    HexPointyTop { hex_size: f32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TurnPhase {
    Lobby,
    SceneIntro,
    IntentCapture,
    RuleResolution,
    RollReveal,
    OutcomeApply,
    ClueUpdate,
    NextTurn,
    SessionClosed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StateWriteCommand {
    pub aggregate_id: Uuid,
    pub expected_version: ExpectedVersion,
    pub idempotency_key: IdempotencyKey,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ConcurrencyError {
    #[error("concurrent update: expected {expected}, current {current}")]
    VersionConflict { expected: i64, current: i64 },
    #[error("retry exhausted for sqlstate {sqlstate} after {attempts} attempts")]
    RetryExhausted { sqlstate: String, attempts: u8 },
}

pub fn is_retryable_sqlstate(sqlstate: &str) -> bool {
    matches!(sqlstate, "40P01" | "40001" | "55P03")
}

pub fn should_retry_deadlock(attempt: u8, sqlstate: &str, has_idempotency_key: bool) -> bool {
    has_idempotency_key && attempt < 3 && is_retryable_sqlstate(sqlstate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retries_only_retryable_idempotent_commands() {
        assert!(should_retry_deadlock(0, "40P01", true));
        assert!(should_retry_deadlock(2, "55P03", true));
        assert!(!should_retry_deadlock(3, "40P01", true));
        assert!(!should_retry_deadlock(0, "40P01", false));
        assert!(!should_retry_deadlock(0, "23505", true));
    }
}
