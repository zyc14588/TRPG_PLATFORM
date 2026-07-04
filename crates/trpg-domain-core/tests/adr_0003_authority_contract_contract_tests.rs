use trpg_domain_core::adr_0003_authority_contract::{
    fork_locked_authority_contract, record_authority_contract_decision,
    reject_authority_contract_update, validate_adr_0003_contract, ADR_0003_INVARIANTS,
};
use trpg_domain_core::authority_contract::DomainAuthorityContract;
use trpg_domain_core::ddd::{
    ActorRole, AuthorityMode, CommandEnvelope, DomainError, EntityId, EventStore, FactProvenance,
    FormalWritePath, PrincipalScope, ProvenanceKind, Visibility, VisibilityLabel,
};

#[test]
fn adr_0003_authority_contract_rejects_authority_violation_without_event() {
    let contract =
        DomainAuthorityContract::new_locked("campaign_001", AuthorityMode::AiKp, "ai_kp", 1)
            .unwrap();
    let command = CommandEnvelope::governed(
        "human override",
        ActorRole::HumanKeeper,
        AuthorityMode::AiKp,
    );
    let mut store = EventStore::default();

    let error = record_authority_contract_decision(&contract, &mut store, &command).unwrap_err();

    assert_eq!(error, DomainError::AuthorityViolation);
    assert!(store.events().is_empty());
}

#[test]
fn adr_0003_authority_contract_blocks_direct_agent_write_without_event() {
    let contract =
        DomainAuthorityContract::new_locked("campaign_001", AuthorityMode::AiKp, "ai_kp", 1)
            .unwrap();
    let mut command =
        CommandEnvelope::governed("direct write", ActorRole::Workflow, AuthorityMode::AiKp);
    command.write_path = FormalWritePath::DirectAgent;
    let mut store = EventStore::default();

    let error = record_authority_contract_decision(&contract, &mut store, &command).unwrap_err();

    assert_eq!(error, DomainError::PolicyDenied);
    assert!(store.events().is_empty());
}

#[test]
fn adr_0003_authority_contract_keeps_visibility_and_fact_provenance_on_replay() {
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
        CommandEnvelope::governed("ruling", ActorRole::HumanKeeper, AuthorityMode::HumanKp);
    command.visibility = Visibility::new(VisibilityLabel::KeeperOnly);
    command.fact_provenance = provenance.clone();
    let mut store = EventStore::default();

    let event = record_authority_contract_decision(&contract, &mut store, &command).unwrap();
    let child =
        fork_locked_authority_contract(&contract, "campaign_child", AuthorityMode::AiKp, "ai_kp")
            .unwrap();

    assert_eq!(event.fact_provenance, provenance);
    assert_eq!(child.version, 1);
    assert_eq!(store.replay_visible(&PrincipalScope::Keeper).len(), 1);
    assert!(store
        .replay_visible(&PrincipalScope::Player(
            EntityId::new("player_001").unwrap()
        ))
        .is_empty());
}

#[test]
fn adr_0003_authority_contract_requires_locked_fork_only_policy() {
    let mut contract =
        DomainAuthorityContract::new_locked("campaign_001", AuthorityMode::AiKp, "ai_kp", 1)
            .unwrap();

    assert!(ADR_0003_INVARIANTS.contains(&"change_policy_fork_only"));
    validate_adr_0003_contract(&contract).unwrap();
    assert_eq!(
        reject_authority_contract_update(&contract, AuthorityMode::HumanKp, "keeper").unwrap_err(),
        DomainError::AuthorityContractImmutable
    );

    contract.locked = false;
    assert_eq!(
        validate_adr_0003_contract(&contract).unwrap_err(),
        DomainError::AuthorityContractImmutable
    );
}
