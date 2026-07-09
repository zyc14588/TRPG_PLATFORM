use trpg_testing::{record_contract_decision, testing_golden_scenarios_ci};

const GOLDEN_SCENARIO: &str =
    include_str!("../../../fixtures/scenarios/golden_salt_bell.scenario.yaml.md");
const GOLDEN_ACTIONS: &str =
    include_str!("../../../fixtures/actions/golden_salt_bell_action_sequence.v1.json.md");

#[test]
fn testing_golden_scenarios_ci_uses_current_s11_fixtures() {
    record_contract_decision(&testing_golden_scenarios_ci::contract()).expect("recorded");

    let combined = format!("{GOLDEN_SCENARIO}\n{GOLDEN_ACTIONS}");

    for marker in testing_golden_scenarios_ci::required_fixture_markers() {
        assert!(combined.contains(marker), "missing marker {marker}");
    }
    assert!(combined.contains("request_skill_check"));
    assert!(combined.contains("history_deleted"));
}
