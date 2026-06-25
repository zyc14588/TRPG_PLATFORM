use auth::VisibilityScope;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AgentEnvelope<T> {
    pub request_id: Uuid,
    pub room_id: Uuid,
    pub session_id: Option<Uuid>,
    pub visibility_scope: VisibilityScope,
    pub input: T,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AgentAuditRef {
    pub agent_name: String,
    pub evidence_ids: Vec<Uuid>,
    pub payload_preview: Value,
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum AgentError {
    #[error("agent output failed schema validation")]
    SchemaInvalid,
    #[error("agent is not implemented in phase 0")]
    NotImplemented,
}
