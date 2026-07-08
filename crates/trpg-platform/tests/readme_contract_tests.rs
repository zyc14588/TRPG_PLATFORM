use trpg_platform::{
    record_readme_contract, PlatformEvent, PlatformEventStore, RecordReadmeContract,
    PLATFORM_INFRASTRUCTURE_INVARIANTS, PLATFORM_README_RECORDED_EVENT,
};
use trpg_shared_kernel::{ActorRole, AuthorityMode, CommandEnvelope};

#[test]
fn readme_invariants_name_top_level_red_lines() {
    assert!(
        PLATFORM_INFRASTRUCTURE_INVARIANTS.contains(&"business_layer_must_not_call_llm_directly")
    );
    assert!(PLATFORM_INFRASTRUCTURE_INVARIANTS
        .contains(&"formal_decisions_go_through_tool_rules_state_event_log"));
}

#[test]
fn readme_contract_is_evented() {
    let command = CommandEnvelope::governed(
        RecordReadmeContract {
            reviewer: "codex".to_owned(),
        },
        ActorRole::System,
        AuthorityMode::HumanKp,
    );
    let mut store = PlatformEventStore::default();

    let event = record_readme_contract(&mut store, &command).expect("readme contract recorded");

    assert_eq!(event.event_type, PLATFORM_README_RECORDED_EVENT);
    assert!(matches!(
        event.payload,
        PlatformEvent::ReadmeContractRecorded { invariant_count }
            if invariant_count == PLATFORM_INFRASTRUCTURE_INVARIANTS.len()
    ));
}
