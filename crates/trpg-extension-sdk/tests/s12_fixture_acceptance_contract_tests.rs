use trpg_extension_sdk::{
    ExtensionCapability, ExtensionCapabilityGrantSet, ExtensionPolicyGate, SdkCompatibilityReport,
};

const S12_DETAILED_FIXTURE: &str = include_str!(
    "../../../fixtures/stages/detailed/S12_extension_sdk_ui_boundary_expected.current.json.md"
);
const S12_SDK_EVIDENCE: &str = include_str!("../../../evidence/stages/S12/sdk-contract.txt");
const S12_UI_EVIDENCE: &str = include_str!("../../../evidence/stages/S12/ui-role-snapshots.txt");
const S12_DEVELOPER_EVIDENCE: &str =
    include_str!("../../../evidence/stages/S12/developer-boundary.txt");

#[test]
fn s12_fixture_expected_errors_are_implemented_by_sdk_errors() {
    assert_expected_error(
        "extension_state_write_bypass",
        "EXTENSION_STATE_WRITE_FORBIDDEN",
    );
    assert_expected_error("player_sees_developer_debug", "UI_ROLE_BOUNDARY_VIOLATION");
    assert_failure_case(
        "sdk_can_call_llm_directly",
        "EXTENSION_DIRECT_LLM_FORBIDDEN",
    );
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
fn s12_fixture_expected_events_and_records_are_automated() {
    assert_expected_event(
        "ExtensionLoaded",
        Some(("extension_id", "coc7_sample_extension")),
    );
    assert_expected_event("UiRoleBoundaryVerified", None);
    assert_required_record_fields(
        "SdkCompatibilityReport",
        &[
            "extension_id",
            "ruleset_version",
            "tool_schema_version",
            "compatibility_result",
        ],
    );
    assert_required_record_fields(
        "UiSnapshotDiff",
        &["role", "snapshot_hash", "redacted_fields"],
    );
}

#[test]
fn s12_fixture_required_evidence_is_materialized() {
    assert_required_evidence(
        "evidence/stages/S12/sdk-contract.txt",
        S12_SDK_EVIDENCE,
        "status: PASS",
    );
    assert_required_evidence(
        "evidence/stages/S12/ui-role-snapshots.txt",
        S12_UI_EVIDENCE,
        "status: PASS",
    );
    assert_required_evidence(
        "evidence/stages/S12/developer-boundary.txt",
        S12_DEVELOPER_EVIDENCE,
        "status: PASS",
    );
    assert!(S12_UI_EVIDENCE.contains("snapshot_hash: sha256:"));
    assert!(S12_UI_EVIDENCE.contains("redacted_fields:"));
    assert!(S12_UI_EVIDENCE.contains("player_snapshot_contains_restricted_fixture_tokens: false"));
    assert!(S12_DEVELOPER_EVIDENCE.contains("debug_data_redacted: PASS"));
    assert!(S12_DEVELOPER_EVIDENCE.contains("player_visible_restricted_fixture_tokens: 0"));
    assert!(!S12_UI_EVIDENCE.contains("status: BLOCKED"));
    assert!(!S12_DEVELOPER_EVIDENCE.contains("status: BLOCKED"));
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

fn assert_expected_event(event_type: &str, required_pair: Option<(&str, &str)>) {
    let block = object_block_after(S12_DETAILED_FIXTURE, "\"expected_events\"", event_type);
    assert!(block.contains(&format!("\"type\": \"{event_type}\"")));
    if let Some((key, value)) = required_pair {
        assert!(block.contains(&format!("\"{key}\": \"{value}\"")));
    }
}

fn assert_required_record_fields(record: &str, fields: &[&str]) {
    let block = object_block_after(S12_DETAILED_FIXTURE, "\"expected_records\"", record);
    assert!(block.contains(&format!("\"record\": \"{record}\"")));
    for field in fields {
        assert!(
            block.contains(&format!("\"{field}\"")),
            "missing field {field} in {record}"
        );
    }
}

fn assert_expected_error(case: &str, error: &str) {
    let block = object_block_after(S12_DETAILED_FIXTURE, "\"expected_errors\"", case);
    assert!(block.contains(&format!("\"case\": \"{case}\"")));
    assert!(block.contains(&format!("\"error\": \"{error}\"")));
}

fn assert_failure_case(id: &str, expected_error: &str) {
    let block = object_block_after(S12_DETAILED_FIXTURE, "\"failure_cases\"", id);
    assert!(block.contains(&format!("\"id\": \"{id}\"")));
    assert!(block.contains(&format!("\"expected_error\": \"{expected_error}\"")));
}

fn assert_required_evidence(path: &str, content: &str, status: &str) {
    assert!(S12_DETAILED_FIXTURE.contains(&format!("\"{path}\"")));
    assert!(
        content.contains(status),
        "expected {path} to contain {status}"
    );
}

fn object_block_after<'a>(fixture: &'a str, section: &str, needle: &str) -> &'a str {
    let section_start = fixture.find(section).expect("fixture section exists");
    let needle_start = fixture[section_start..]
        .find(needle)
        .map(|offset| section_start + offset)
        .expect("fixture object exists");
    let object_start = fixture[..needle_start].rfind('{').expect("object starts");
    let object_end = fixture[needle_start..]
        .find('}')
        .map(|offset| needle_start + offset + 1)
        .expect("object ends");
    &fixture[object_start..object_end]
}
