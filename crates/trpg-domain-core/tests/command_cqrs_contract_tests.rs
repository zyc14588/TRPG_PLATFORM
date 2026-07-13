use trpg_domain_core::authority_contract::DomainAuthorityContract;
use trpg_domain_core::command_cqrs::{submit_domain_command, DomainCommandKind};
use trpg_domain_core::ddd::{ActorRole, AuthorityMode, EventStore, FactSource, FormalWritePath};

#[test]
fn command_cqrs_commits_formal_commands_as_event_store_entries() {
    let contract = DomainAuthorityContract::new_locked(
        "camp_human_archive",
        AuthorityMode::HumanKp,
        "user_human_kp",
        1,
    )
    .unwrap();
    let command = trpg_test_support::governed_command!(
        "payload",
        ActorRole::HumanKeeper,
        AuthorityMode::HumanKp
    );
    let mut store = EventStore::default();

    let event = submit_domain_command(
        &contract,
        &mut store,
        &command,
        DomainCommandKind::RecordDecision,
        FactSource::DecisionRecord,
    )
    .unwrap();

    assert_eq!(event.sequence, 1);
    assert_eq!(event.event_type, "CommandAccepted");
    assert_eq!(store.events().len(), 1);
}

#[test]
fn command_cqrs_rejects_direct_agent_state_write() {
    let contract = DomainAuthorityContract::new_locked(
        "camp_ai_harbor",
        AuthorityMode::AiKp,
        "ai_kp_local_level4",
        1,
    )
    .unwrap();
    let mut command =
        trpg_test_support::governed_command!("payload", ActorRole::System, AuthorityMode::AiKp);
    command.write_path = FormalWritePath::DirectAgent;
    let mut store = EventStore::default();

    let error = submit_domain_command(
        &contract,
        &mut store,
        &command,
        DomainCommandKind::RecordDecision,
        FactSource::DecisionRecord,
    )
    .unwrap_err();

    assert_eq!(error.code(), "POLICY_DENIED");
    assert!(store.events().is_empty());
}
