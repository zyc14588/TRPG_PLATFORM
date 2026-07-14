use trpg_domain_core::ddd::{
    ActorRole, AuthorityMode, DomainError, EntityId, EventStore, FactProvenance, FactSource,
    PrincipalScope, ProvenanceKind, Visibility, VisibilityLabel,
};
use trpg_domain_core::rule_runtime_coc7_impl::{
    record_rule_runtime_coc7_decision, Coc7RuleRuntimeDecision,
};

#[test]
fn rule_runtime_coc7_impl_rejects_authority_violation_without_event() {
    let contract = trpg_test_support::authority_contract_with_owner(
        "campaign_001",
        AuthorityMode::AiKp,
        "ai_kp",
        1,
    )
    .unwrap();
    let command = trpg_test_support::governed_command_for_contract(
        &contract,
        "skill check",
        ActorRole::HumanKeeper,
    );
    let mut store = EventStore::default();

    let error = record_rule_runtime_coc7_decision(
        &contract,
        &mut store,
        &command,
        Coc7RuleRuntimeDecision::SkillCheck,
    )
    .unwrap_err();

    assert_eq!(error, DomainError::AuthorityViolation);
    assert!(store.events().is_empty());
}

#[test]
fn rule_runtime_coc7_impl_preserves_visibility_and_provenance_on_replay() {
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
        "sanity check",
        ActorRole::HumanKeeper,
    );
    command.visibility = Visibility::new(VisibilityLabel::KeeperOnly);
    command.fact_provenance = provenance.clone();
    let mut store = EventStore::default();

    let event = record_rule_runtime_coc7_decision(
        &contract,
        &mut store,
        &command,
        Coc7RuleRuntimeDecision::SanityCheck,
    )
    .unwrap();

    assert_eq!(event.fact_provenance, provenance);
    assert_eq!(event.payload.fact_source, FactSource::DiceRoll);
    assert_eq!(store.replay_visible(&PrincipalScope::Keeper).len(), 1);
    assert!(store
        .replay_visible(&PrincipalScope::Player(
            EntityId::new("player_001").unwrap()
        ))
        .is_empty());
}
