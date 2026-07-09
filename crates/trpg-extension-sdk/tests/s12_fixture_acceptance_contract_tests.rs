use trpg_extension_sdk::{
    ExtensionCapability, ExtensionCapabilityGrantSet, ExtensionPolicyGate, SdkCompatibilityReport,
};

#[test]
fn s12_fixture_expected_errors_are_implemented_by_sdk_errors() {
    let detailed_fixture = include_str!(
        "../../../fixtures/stages/detailed/S12_extension_sdk_ui_boundary_expected.current.json.md"
    );

    assert!(detailed_fixture.contains("EXTENSION_STATE_WRITE_FORBIDDEN"));
    assert!(detailed_fixture.contains("EXTENSION_DIRECT_LLM_FORBIDDEN"));
    assert_eq!(
        ExtensionCapabilityGrantSet::with_grants(&[ExtensionCapability::AppendEventStore])
            .expect_err("extension cannot request direct Event Store append")
            .code(),
        "EXTENSION_STATE_WRITE_FORBIDDEN"
    );
    assert_eq!(
        ExtensionCapabilityGrantSet::with_grants(&[ExtensionCapability::DirectLlm])
            .expect_err("extension cannot call LLM directly")
            .code(),
        "EXTENSION_DIRECT_LLM_FORBIDDEN"
    );
}

#[test]
fn s12_api_fixture_keeps_direct_llm_subject_denied() {
    let api_fixture = include_str!("../../../fixtures/api/api_ws_nats_contract_cases.v1.json.md");

    assert!(api_fixture.contains("trpg.llm.direct.*"));
    assert!(
        ExtensionPolicyGate::default_deny(&[ExtensionCapability::InvokeGrantedTool])
            .authorize()
            .is_err()
    );
}

#[test]
fn s12_compatibility_report_has_required_acceptance_fields() {
    let stage_fixture =
        include_str!("../../../fixtures/stages/S12_stage_acceptance_fixture.v1.json.md");
    let report =
        SdkCompatibilityReport::compatible("coc7_sample_extension", "7e", "tool_schema.v1");

    assert!(stage_fixture.contains("S12"));
    assert!(report.has_required_fields());
}
