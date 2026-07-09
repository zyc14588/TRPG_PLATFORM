use trpg_agent_runtime::{provider_boundary_snapshot, AgentError};
use trpg_testing::{
    model_certification_tests, record_contract_decision, TESTING_QUALITY_DECISION_RECORDED_EVENT,
};

const MODEL_CERTIFICATION_FIXTURE: &str =
    include_str!("../../../test-data/provider_model_certification_cases.md");

#[test]
fn model_certification_requires_level4_for_ai_keeper() {
    let (_, _, event) =
        record_contract_decision(&model_certification_tests::contract()).expect("recorded");

    assert_eq!(event.event_type, TESTING_QUALITY_DECISION_RECORDED_EVENT);
    assert!(MODEL_CERTIFICATION_FIXTURE.contains("LOCAL_MODEL_LEVEL_4"));
    assert!(MODEL_CERTIFICATION_FIXTURE.contains("DENY_AND_AUDIT"));
    assert!(model_certification_tests::level4_is_required_for_ai_keeper());
}

#[test]
fn model_certification_denies_silent_cloud_fallback() {
    let boundary = provider_boundary_snapshot();

    assert_eq!(boundary.gateway, "Agent Gateway");
    assert_eq!(
        boundary.forbidden_direct_call_error,
        AgentError::DirectLlmCallForbidden.code()
    );
    assert!(model_certification_tests::silent_cloud_fallback_is_denied());
    assert!(model_certification_tests::explicit_cloud_fallback_is_allowed());
}
