mod common;

use trpg_ruleset_coc7::coc7_rules_engine::{
    engine_decision_route, record_coc7_rules_engine_decision, Coc7EngineDecision,
};

#[test]
fn engine_routes_known_coc7_decisions() {
    assert_eq!(
        engine_decision_route(Coc7EngineDecision::SanityCheck),
        "sanity_check"
    );
}

#[test]
fn engine_decision_is_event_logged() {
    let contract = common::human_contract();
    let mut store = common::event_store();
    let command = common::rules_command("engine");

    let event = record_coc7_rules_engine_decision(
        &contract,
        &mut store,
        &command,
        Coc7EngineDecision::CombatRound,
    )
    .unwrap();

    assert_eq!(event.event_type, "CombatStateUpdated");
    assert_eq!(event.payload.schema_version, 1);
}

#[test]
fn unregistered_event_is_rejected_before_event_store_append() {
    let contract = common::human_contract();
    let mut store = common::event_store();
    let command = common::rules_command("unknown");

    let error = trpg_ruleset_coc7::append_coc7_event(
        &contract,
        &mut store,
        &command,
        "UnregisteredEvent",
        "negative_contract_test",
        "must not append",
    )
    .unwrap_err();

    assert_eq!(error.code(), "EVENT_CONTRACT_UNKNOWN");
    assert!(store.events().is_empty());
}
