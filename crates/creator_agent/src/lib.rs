use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StoryDraft {
    pub title: String,
    pub spoiler_free: bool,
}

pub fn phase0_not_implemented() -> agent_core::AgentError {
    agent_core::AgentError::NotImplemented
}
