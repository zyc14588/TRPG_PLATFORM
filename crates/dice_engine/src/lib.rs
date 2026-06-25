use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DiceFormula(pub String);

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum DiceError {
    #[error("dice formula cannot be empty")]
    EmptyFormula,
}

impl DiceFormula {
    pub fn parse(input: &str) -> Result<Self, DiceError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(DiceError::EmptyFormula);
        }

        Ok(Self(trimmed.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_rejects_empty_formula() {
        assert_eq!(DiceFormula::parse(""), Err(DiceError::EmptyFormula));
        assert_eq!(
            DiceFormula::parse(" 1d20 ").map(|f| f.0),
            Ok("1d20".to_owned())
        );
    }
}
