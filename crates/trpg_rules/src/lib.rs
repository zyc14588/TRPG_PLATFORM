use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ValidationReport {
    pub is_valid: bool,
    pub messages: Vec<String>,
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum RuleError {
    #[error("rule system has no bundled commercial text")]
    NoBundledCommercialText,
    #[error("rule system is not implemented in phase 0")]
    NotImplemented,
}

pub trait RuleSystem: Send + Sync {
    fn id(&self) -> &'static str;
    fn character_schema(&self) -> Value;
    fn validate_character(&self, sheet: &Value) -> Result<ValidationReport, RuleError>;
}

pub fn empty_character_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": true
    })
}
