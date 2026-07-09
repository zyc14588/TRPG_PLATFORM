use trpg_testing::{record_contract_decision, testing_golden_ci};

const S11_STAGE_FIXTURE: &str =
    include_str!("../../../fixtures/stages/S11_stage_acceptance_fixture.v1.json.md");
const S11_DETAILED_FIXTURE: &str = include_str!(
    "../../../fixtures/stages/detailed/S11_golden_visibility_export_diff_expected.current.json.md"
);

#[test]
fn testing_golden_ci_lists_required_stage_gates() {
    record_contract_decision(&testing_golden_ci::contract()).expect("recorded");

    assert!(S11_STAGE_FIXTURE.contains("S11"));
    assert!(S11_DETAILED_FIXTURE.contains("golden_scenario_passes"));
    assert!(S11_DETAILED_FIXTURE.contains("no_visibility_leakage"));
    assert!(S11_DETAILED_FIXTURE.contains("eval_report_written"));

    let gates = testing_golden_ci::required_gates();

    assert!(gates
        .iter()
        .any(|gate| gate.command == "cargo test -p trpg-testing --all-features"));
    assert!(gates.iter().any(|gate| gate.name == "model_certification"));
}
