use dice_engine::DiceFormula;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DiceAgentOutput {
    pub needs_roll: bool,
    pub dice_formula: Option<DiceFormula>,
    pub uncertainty: Option<String>,
}

pub fn phase0_not_implemented() -> agent_core::AgentError {
    agent_core::AgentError::NotImplemented
}
