use trpg_shared_kernel::rust_coding_model::{
    validate_rust_symbol_name, validate_schema_type, SchemaBoundary,
};
use trpg_shared_kernel::TrpgError;

#[test]
fn rust_coding_model_rejects_template_names() {
    let template_name = format!("Module{}", "Service");

    assert!(matches!(
        validate_rust_symbol_name(&template_name),
        Err(TrpgError::CodingPolicyViolation(_))
    ));
}

#[test]
fn rust_coding_model_rejects_source_hash_fragments() {
    assert!(matches!(
        validate_rust_symbol_name("cargo_workspace_47cddc714f"),
        Err(TrpgError::CodingPolicyViolation(_))
    ));
}

#[test]
fn rust_coding_model_limits_untyped_json_to_schema_boundary() {
    let untyped_json = format!("serde_json::{}", "Value");

    assert!(matches!(
        validate_schema_type(SchemaBoundary::DomainModel, &untyped_json),
        Err(TrpgError::CodingPolicyViolation(_))
    ));

    validate_schema_type(SchemaBoundary::ExternalSchema, &untyped_json).unwrap();
}
