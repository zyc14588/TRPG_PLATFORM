use trpg_domain_core::authority_contract::DomainAuthorityContract;
use trpg_domain_core::character_combat_san_chase::{
    record_character_combat_san_chase_decision, CharacterCombatSanChaseDecision,
    CharacterCombatSanChaseTrack,
};
use trpg_domain_core::ddd::{
    ActorRole, AuthorityMode, CommandEnvelope, DomainError, EntityId, EventStore, FactProvenance,
    PrincipalScope, ProvenanceKind, Visibility, VisibilityLabel,
};

#[test]
fn character_combat_san_chase_rejects_authority_violation_without_event() {
    let contract =
        DomainAuthorityContract::new_locked("campaign_001", AuthorityMode::AiKp, "ai_kp", 1)
            .unwrap();
    let command =
        CommandEnvelope::governed("combat ruling", ActorRole::HumanKeeper, AuthorityMode::AiKp);
    let decision = CharacterCombatSanChaseDecision::for_track(CharacterCombatSanChaseTrack::Combat);
    let mut store = EventStore::default();

    let error =
        record_character_combat_san_chase_decision(&contract, &mut store, &command, decision)
            .unwrap_err();

    assert_eq!(error, DomainError::AuthorityViolation);
    assert!(store.events().is_empty());
}

#[test]
fn character_combat_san_chase_keeps_visibility_and_fact_provenance_on_replay() {
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
        "sanity loss",
        ActorRole::HumanKeeper,
        AuthorityMode::HumanKp,
    );
    command.visibility = Visibility::new(VisibilityLabel::KeeperOnly);
    command.fact_provenance = provenance.clone();
    let decision = CharacterCombatSanChaseDecision::for_track(CharacterCombatSanChaseTrack::Sanity);
    let mut store = EventStore::default();

    let event =
        record_character_combat_san_chase_decision(&contract, &mut store, &command, decision)
            .unwrap();

    assert_eq!(event.fact_provenance, provenance);
    assert_eq!(event.payload.fact_source, decision.fact_source);
    assert_eq!(store.replay_visible(&PrincipalScope::Keeper).len(), 1);
    assert!(store
        .replay_visible(&PrincipalScope::Player(
            EntityId::new("player_001").unwrap()
        ))
        .is_empty());
}
