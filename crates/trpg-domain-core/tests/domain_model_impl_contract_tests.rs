use trpg_domain_core::command_cqrs::DomainCommandKind;
use trpg_domain_core::ddd::{
    ActorRole, AuthorityMode, DomainError, EntityId, EventStore, FactProvenance, FactSource,
    FormalWritePath, PrincipalScope, ProvenanceKind, Visibility, VisibilityLabel,
};
use trpg_domain_core::domain_model::DomainModelCommand;
use trpg_domain_core::domain_model_impl::{
    accept_domain_model_command, reject_ungoverned_domain_write,
};

#[test]
fn domain_model_impl_rejects_direct_agent_write_without_event() {
    let mut command =
        trpg_test_support::governed_command("draft", ActorRole::Workflow, AuthorityMode::AiKp);
    command.write_path = FormalWritePath::DirectAgent;

    assert_eq!(
        reject_ungoverned_domain_write(&command).unwrap_err(),
        DomainError::PolicyDenied
    );
}

#[test]
fn domain_model_impl_preserves_visibility_and_provenance_on_replay() {
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
    let mut envelope = trpg_test_support::governed_command_for_contract(
        &contract,
        "domain action",
        ActorRole::HumanKeeper,
    );
    envelope.visibility = Visibility::new(VisibilityLabel::KeeperOnly);
    envelope.fact_provenance = provenance.clone();
    let model_command = DomainModelCommand {
        kind: DomainCommandKind::SubmitPlayerAction,
        fact_source: FactSource::DecisionRecord,
        envelope,
    };
    let mut store = EventStore::default();

    let event = accept_domain_model_command(&contract, &mut store, &model_command).unwrap();

    assert_eq!(event.fact_provenance, provenance);
    assert_eq!(store.replay_visible(&PrincipalScope::Keeper).len(), 1);
    assert!(store
        .replay_visible(&PrincipalScope::Player(
            EntityId::new("player_001").unwrap()
        ))
        .is_empty());
}
