use serde_json::Value;
use trpg_rules::{empty_character_schema, RuleError, RuleSystem, ValidationReport};

#[derive(Debug, Clone, Default)]
pub struct GenericPercentileRules;

impl RuleSystem for GenericPercentileRules {
    fn id(&self) -> &'static str {
        "generic_percentile"
    }

    fn character_schema(&self) -> Value {
        empty_character_schema()
    }

    fn validate_character(&self, _sheet: &Value) -> Result<ValidationReport, RuleError> {
        Ok(ValidationReport {
            is_valid: true,
            messages: Vec::new(),
        })
    }
}
