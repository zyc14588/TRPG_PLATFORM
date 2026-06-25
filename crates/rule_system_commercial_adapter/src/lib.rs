use serde_json::Value;
use trpg_rules::{empty_character_schema, RuleError, RuleSystem, ValidationReport};

#[derive(Debug, Clone, Default)]
pub struct CommercialRulesAdapter;

impl RuleSystem for CommercialRulesAdapter {
    fn id(&self) -> &'static str {
        "licensed_commercial"
    }

    fn character_schema(&self) -> Value {
        empty_character_schema()
    }

    fn validate_character(&self, _sheet: &Value) -> Result<ValidationReport, RuleError> {
        Err(RuleError::NoBundledCommercialText)
    }
}
