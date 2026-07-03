use crate::shared_kernel::{KernelResult, TrpgError};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SchemaBoundary {
    DomainModel,
    ExternalSchema,
}

pub fn validate_rust_symbol_name(name: &str) -> KernelResult<()> {
    let template_tokens = [
        format!("Module{}", "Service"),
        format!("Module{}", "Command"),
        format!("Module{}", "Error"),
    ];

    if template_tokens.iter().any(|token| name.contains(token)) {
        return Err(TrpgError::CodingPolicyViolation(
            "template module names are not final implementation names",
        ));
    }

    if name
        .split(|c: char| !c.is_ascii_hexdigit())
        .any(|part| part.len() >= 10 && part.chars().all(|c| c.is_ascii_hexdigit()))
    {
        return Err(TrpgError::CodingPolicyViolation(
            "source hash fragments must not be used as Rust names",
        ));
    }

    Ok(())
}

pub fn validate_schema_type(boundary: SchemaBoundary, type_name: &str) -> KernelResult<()> {
    let is_untyped_json = type_name.contains("serde_json::") && type_name.ends_with("Value");
    if boundary != SchemaBoundary::ExternalSchema && is_untyped_json {
        return Err(TrpgError::CodingPolicyViolation(
            "untyped JSON is allowed only at external schema boundaries",
        ));
    }

    Ok(())
}
