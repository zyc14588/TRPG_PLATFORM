use trpg_testing::{golden_scenario_ci, record_contract_decision};

const GOLDEN_ACTIONS: &str =
    include_str!("../../../fixtures/actions/golden_salt_bell_action_sequence.v1.json.md");

#[test]
fn golden_scenario_ci_keeps_formal_decisions_on_event_path() {
    record_contract_decision(&golden_scenario_ci::contract()).expect("recorded");

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
    assert!(GOLDEN_ACTIONS.contains("dice_source"));
    assert!(GOLDEN_ACTIONS.contains("history_deleted"));
}
