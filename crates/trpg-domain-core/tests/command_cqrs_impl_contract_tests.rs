use trpg_domain_core::authority_contract::DomainAuthorityContract;
use trpg_domain_core::command_cqrs::DomainCommandKind;
use trpg_domain_core::command_cqrs_impl::{command_accepted_decision, commit_governed_command};
use trpg_domain_core::ddd::{
    ActorRole, AuthorityMode, CommandEnvelope, DomainError, EntityId, EventStore, FactProvenance,
    FactSource, PrincipalScope, ProvenanceKind, Visibility, VisibilityLabel,
};

#[test]
fn command_cqrs_impl_rejects_authority_violation_without_event() {
    let contract =
        DomainAuthorityContract::new_locked("campaign_001", AuthorityMode::AiKp, "ai_kp", 1)
            .unwrap();
    let command = CommandEnvelope::governed("command", ActorRole::HumanKeeper, AuthorityMode::AiKp);
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
    let contract =
        DomainAuthorityContract::new_locked("campaign_001", AuthorityMode::HumanKp, "keeper", 1)
            .unwrap();
    let provenance = FactProvenance::new(
        ProvenanceKind::RulesEngineDecision,
        "event_001",
        "rules_001",
    )
    .unwrap();
    let mut command =
        CommandEnvelope::governed("command", ActorRole::HumanKeeper, AuthorityMode::HumanKp);
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
