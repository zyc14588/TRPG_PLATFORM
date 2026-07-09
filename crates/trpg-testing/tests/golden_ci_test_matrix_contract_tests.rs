use trpg_testing::{golden_ci_test_matrix, record_contract_decision};

const S11_TEST_PLAN: &str =
    include_str!("../../../stages/s11-testing-quality-golden-ci/TEST_PLAN.md");
const S11_ACCEPTANCE_FIXTURE: &str =
    include_str!("../../../fixtures/stages/S11_stage_acceptance_fixture.v1.json.md");
const V1_ACCEPTANCE: &str = include_str!("../../../V1_ACCEPTANCE_EVIDENCE_MATRIX.md");

#[test]
fn golden_ci_test_matrix_requires_s11_governance_gates() {
    record_contract_decision(&golden_ci_test_matrix::contract()).expect("recorded");

    assert!(golden_ci_test_matrix::covers_gate("golden_scenarios_ci"));
    assert!(golden_ci_test_matrix::covers_gate("visibility_leakage"));
    assert!(golden_ci_test_matrix::covers_gate(
        "model_certification_tests"
    ));
    assert!(S11_TEST_PLAN.contains("cargo test -p trpg-testing --all-features"));
    assert!(S11_ACCEPTANCE_FIXTURE.contains("may_weaken_tests"));
    assert!(V1_ACCEPTANCE.contains("golden_scenarios_ci"));
}
