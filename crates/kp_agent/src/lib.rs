use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct KpVisibleOutput {
    pub visible_narration: String,
    pub uncertainty: Option<String>,
}

pub fn phase0_not_implemented() -> agent_core::AgentError {
    agent_core::AgentError::NotImplemented
}
