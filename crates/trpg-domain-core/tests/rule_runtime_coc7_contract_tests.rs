use trpg_domain_core::authority_contract::DomainAuthorityContract;
use trpg_domain_core::ddd::{
    ActorRole, AuthorityMode, CommandEnvelope, DomainError, EntityId, EventStore, FactProvenance,
    FactSource, PrincipalScope, ProvenanceKind, Visibility, VisibilityLabel,
};
use trpg_domain_core::rule_runtime_coc7::{
    record_rule_runtime_coc7_decision, Coc7RuleRuntimeDecision,
};

#[test]
fn rule_runtime_coc7_rejects_authority_violation_without_event() {
    let contract =
        DomainAuthorityContract::new_locked("campaign_001", AuthorityMode::AiKp, "ai_kp", 1)
            .unwrap();
    let command =
        CommandEnvelope::governed("skill check", ActorRole::HumanKeeper, AuthorityMode::AiKp);
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
fn rule_runtime_coc7_keeps_visibility_and_fact_provenance_on_replay() {
    let contract =
        DomainAuthorityContract::new_locked("campaign_001", AuthorityMode::HumanKp, "keeper", 1)
            .unwrap();
    let provenance = FactProvenance::new(
        ProvenanceKind::RulesEngineDecision,
        "event_001",
        "rules_001",
    )
    .unwrap();
    let mut command = CommandEnvelope::governed(
        "sanity check",
        ActorRole::HumanKeeper,
        AuthorityMode::HumanKp,
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
