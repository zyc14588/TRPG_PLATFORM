use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use trpg_rules::ValidationReport;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CharacterAgentOutput {
    pub validation: ValidationReport,
    pub uncertainty: Option<String>,
}

pub fn phase0_not_implemented() -> agent_core::AgentError {
    agent_core::AgentError::NotImplemented
}
