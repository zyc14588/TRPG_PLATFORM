use trpg_testing::{record_contract_decision, runtime_pending_decision};

const GOLDEN_ACTIONS: &str =
    include_str!("../../../fixtures/actions/golden_salt_bell_action_sequence.v1.json.md");
const TOP_LEVEL_DESIGN: &str =
    include_str!("../../../docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md");

#[test]
fn runtime_pending_decision_blocks_agent_state_writes_until_event_store() {
    record_contract_decision(&runtime_pending_decision::contract()).expect("recorded");

    assert!(runtime_pending_decision::blocks_direct_agent_write());
    assert!(runtime_pending_decision::pending_decision_rules()
        .iter()
        .any(|rule| rule.required_next_step == "event_store_append"));
    assert!(GOLDEN_ACTIONS.contains("EventStore") || GOLDEN_ACTIONS.contains("event"));
    assert!(TOP_LEVEL_DESIGN.contains("Authority Contract"));
}
