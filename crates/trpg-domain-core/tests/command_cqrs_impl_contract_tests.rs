use trpg_domain_core::command_cqrs::DomainCommandKind;
use trpg_domain_core::command_cqrs_impl::{command_accepted_decision, commit_governed_command};
use trpg_domain_core::ddd::{
    ActorRole, AuthorityMode, DomainError, EntityId, EventStore, FactProvenance, FactSource,
    PrincipalScope, ProvenanceKind, Visibility, VisibilityLabel,
};

#[test]
fn command_cqrs_impl_rejects_authority_violation_without_event() {
    let contract = trpg_test_support::authority_contract_with_owner(
        "campaign_001",
        AuthorityMode::AiKp,
        "ai_kp",
        1,
    )
    .unwrap();
    let command = trpg_test_support::governed_command_for_contract(
        &contract,
        "command",
        ActorRole::HumanKeeper,
    );
    let decision = command_accepted_decision(
        DomainCommandKind::RecordDecision,
        FactSource::DecisionRecord,
    );
    let mut store = EventStore::default();

    let error = commit_governed_command(&contract, &mut store, &command, decision).unwrap_err();

    assert_eq!(error, DomainError::AuthorityViolation);
    assert!(store.events().is_empty());
}

#[test]
fn command_cqrs_impl_preserves_visibility_and_provenance_on_replay() {
    let contract = trpg_test_support::authority_contract_with_owner(
        "campaign_001",
        AuthorityMode::HumanKp,
        "keeper",
        1,
    )
    .unwrap();
    let provenance = FactProvenance::new(
        ProvenanceKind::RulesEngineDecision,
        "event_001",
        "rules_001",
    )
    .unwrap();
    let mut command = trpg_test_support::governed_command_for_contract(
        &contract,
        "command",
        ActorRole::HumanKeeper,
    );
    command.visibility = Visibility::new(VisibilityLabel::KeeperOnly);
    command.fact_provenance = provenance.clone();
    let decision = command_accepted_decision(
        DomainCommandKind::RecordDecision,
        FactSource::DecisionRecord,
    );
    let mut store = EventStore::default();

    let event = commit_governed_command(&contract, &mut store, &command, decision).unwrap();

    assert_eq!(event.fact_provenance, provenance);
    assert_eq!(event.payload.fact_source, FactSource::DecisionRecord);
    assert_eq!(store.replay_visible(&PrincipalScope::Keeper).len(), 1);
    assert!(store
        .replay_visible(&PrincipalScope::Player(
            EntityId::new("player_001").unwrap()
        ))
        .is_empty());
}
