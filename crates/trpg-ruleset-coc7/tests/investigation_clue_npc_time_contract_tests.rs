mod common;

use trpg_ruleset_coc7::investigation_clue_npc_time::{
    advance_time, record_investigation_clue_npc_time_decision, resolve_clue_check, ClueImportance,
    ClueOutcome,
};

#[test]
fn core_clue_failure_reveals_with_cost() {
    let resolution = resolve_clue_check(ClueImportance::Core, false);

    assert_eq!(resolution.outcome, ClueOutcome::RevealedWithCost);
    assert_eq!(resolution.cost, Some("time_or_complication"));
}

#[test]
fn time_and_clue_decision_are_governed_events() {
    let time = advance_time(30, 15, "library search").unwrap();
    assert_eq!(time.after_minutes, 45);

    let contract = common::human_contract();
    let mut store = common::event_store();
    let command = common::rules_command("clue");
    let resolution = resolve_clue_check(ClueImportance::Optional, false);

    let event =
        record_investigation_clue_npc_time_decision(&contract, &mut store, &command, &resolution)
            .unwrap();

    assert_eq!(
        event.event_type,
        "coc7.investigation_clue_npc_time_recorded"
    );
}
