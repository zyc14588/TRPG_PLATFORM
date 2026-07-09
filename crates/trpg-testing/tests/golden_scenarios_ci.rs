use trpg_testing::{golden_scenario_ci, golden_scenarios_ci, testing_golden_scenarios_ci};

const GOLDEN_SCENARIO: &str =
    include_str!("../../../fixtures/scenarios/golden_salt_bell.scenario.yaml.md");
const GOLDEN_ACTIONS: &str =
    include_str!("../../../fixtures/actions/golden_salt_bell_action_sequence.v1.json.md");
const S11_EXPECTED: &str = include_str!(
    "../../../fixtures/stages/detailed/S11_golden_visibility_export_diff_expected.current.json.md"
);

#[test]
fn golden_scenarios_ci_stage_gate() {
    let combined = format!("{GOLDEN_SCENARIO}\n{GOLDEN_ACTIONS}\n{S11_EXPECTED}");

    for marker in testing_golden_scenarios_ci::required_fixture_markers() {
        assert!(combined.contains(marker), "missing marker {marker}");
    }
    for marker in golden_scenarios_ci::required_fixture_markers() {
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
    assert!(golden_scenarios_ci::requires_event_store_path());
    assert!(golden_scenarios_ci::requires_server_dice());
    assert!(golden_scenarios_ci::requires_visibility_redaction());
}
