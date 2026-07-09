use trpg_testing::model_certification_tests;

const MODEL_CERTIFICATION_FIXTURE: &str =
    include_str!("../../../test-data/provider_model_certification_cases.md");

#[test]
fn model_certification_stage_gate() {
    assert!(MODEL_CERTIFICATION_FIXTURE.contains("LOCAL_MODEL_LEVEL_4"));
    assert!(MODEL_CERTIFICATION_FIXTURE.contains("DENY_AND_AUDIT"));
    assert!(model_certification_tests::level4_is_required_for_ai_keeper());
    assert!(model_certification_tests::silent_cloud_fallback_is_denied());
    assert!(model_certification_tests::explicit_cloud_fallback_is_allowed());
}
