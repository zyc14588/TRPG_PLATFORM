use serde_json::Value;
use trpg_rules::{empty_character_schema, RuleError, RuleSystem, ValidationReport};

#[derive(Debug, Clone, Default)]
pub struct Dnd5eSrdRules;

impl RuleSystem for Dnd5eSrdRules {
    fn id(&self) -> &'static str {
        "dnd5e_srd_5_2_1"
    }

    fn character_schema(&self) -> Value {
        empty_character_schema()
    }

    fn validate_character(&self, _sheet: &Value) -> Result<ValidationReport, RuleError> {
        Ok(ValidationReport {
            is_valid: true,
            messages: vec!["Phase 0 adapter only; no SRD text is bundled here.".to_owned()],
        })
    }
}
