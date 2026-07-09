use trpg_testing::{record_contract_decision, requirement_to_test_trace};

const V1_ACCEPTANCE: &str = include_str!("../../../V1_ACCEPTANCE_EVIDENCE_MATRIX.md");
const S11_TEST_PLAN: &str =
    include_str!("../../../stages/s11-testing-quality-golden-ci/TEST_PLAN.md");

#[test]
fn requirement_to_test_trace_links_v1_requirements_to_commands_and_evidence() {
    record_contract_decision(&requirement_to_test_trace::contract()).expect("recorded");

    assert!(requirement_to_test_trace::has_test_for(
        "complete_tutorial_scenario"
    ));
    assert!(requirement_to_test_trace::has_test_for(
        "visibility_leakage_blocked"
    ));
    assert!(requirement_to_test_trace::has_test_for(
        "model_certification_enforced"
    ));
    assert!(requirement_to_test_trace::requirement_links()
        .iter()
        .all(|link| !link.evidence_path.is_empty()));
    assert!(V1_ACCEPTANCE.contains("Golden Scenario"));
    assert!(S11_TEST_PLAN.contains("provider_model_certification_cases"));
}
