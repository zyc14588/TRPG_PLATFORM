use trpg_testing::{golden_scenario_ci, testing_golden_scenarios_ci};

const GOLDEN_SCENARIO: &str =
    include_str!("../../../fixtures/scenarios/golden_salt_bell.scenario.yaml.md");
const GOLDEN_ACTIONS: &str =
    include_str!("../../../fixtures/actions/golden_salt_bell_action_sequence.v1.json.md");

#[test]
fn golden_scenarios_ci_stage_gate() {
    let combined = format!("{GOLDEN_SCENARIO}\n{GOLDEN_ACTIONS}");

    for marker in testing_golden_scenarios_ci::required_fixture_markers() {
        assert!(combined.contains(marker), "missing marker {marker}");
    }
    assert_eq!(
        golden_scenario_ci::official_decision_pipeline(),
        &[
            "Command",
            "Workflow",
            "Decision",
            "EventStore",
            "Projection"
        ]
    );
    assert!(golden_scenario_ci::server_dice_required());
}
