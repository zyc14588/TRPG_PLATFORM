use trpg_domain_core::authority_contract::DomainAuthorityContract;
use trpg_domain_core::authority_contract_impl::{
    append_authority_contract_decision, fork_locked_authority_contract,
};
use trpg_domain_core::ddd::{
    ActorRole, AuthorityMode, DomainError, EntityId, EventStore, FactProvenance, PrincipalScope,
    ProvenanceKind, Visibility, VisibilityLabel,
};

#[test]
fn authority_contract_impl_rejects_authority_violation_without_event() {
    let contract =
        DomainAuthorityContract::new_locked("campaign_001", AuthorityMode::AiKp, "ai_kp", 1)
            .unwrap();
    let command = trpg_test_support::governed_command!(
        "human override",
        ActorRole::HumanKeeper,
        AuthorityMode::AiKp,
    );
    let mut store = EventStore::default();

    let error = append_authority_contract_decision(&contract, &mut store, &command).unwrap_err();

    assert_eq!(error, DomainError::AuthorityViolation);
    assert!(store.events().is_empty());
}

#[test]
fn authority_contract_impl_preserves_visibility_and_provenance_on_replay() {
    let contract =
        DomainAuthorityContract::new_locked("campaign_001", AuthorityMode::HumanKp, "keeper", 1)
            .unwrap();
    let provenance = FactProvenance::new(
        ProvenanceKind::RulesEngineDecision,
        "event_001",
        "rules_001",
    )
    .unwrap();
    let mut command = trpg_test_support::governed_command!(
        "ruling",
        ActorRole::HumanKeeper,
        AuthorityMode::HumanKp
    );
    command.visibility = Visibility::new(VisibilityLabel::KeeperOnly);
    command.fact_provenance = provenance.clone();
    let mut store = EventStore::default();

    let event = append_authority_contract_decision(&contract, &mut store, &command).unwrap();
    let child =
        fork_locked_authority_contract(&contract, "campaign_child", AuthorityMode::AiKp, "ai_kp")
            .unwrap();

    assert_eq!(event.fact_provenance, provenance);
    assert_eq!(child.version(), 1);
    assert_eq!(store.replay_visible(&PrincipalScope::Keeper).len(), 1);
    assert!(store
        .replay_visible(&PrincipalScope::Player(
            EntityId::new("player_001").unwrap()
        ))
        .is_empty());
}
