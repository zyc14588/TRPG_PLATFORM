use trpg_domain_core::authority_contract::DomainAuthorityContract;
use trpg_domain_core::ddd::{
    ActorRole, AuthorityMode, CommandEnvelope, DomainError, EntityId, EventStore, FactProvenance,
    PrincipalScope, ProvenanceKind, Visibility, VisibilityLabel,
};
use trpg_domain_core::investigation_clue_npc_time::{
    record_investigation_clue_npc_time_decision, InvestigationClueNpcTimeDecision,
    InvestigationClueNpcTimeTrack,
};

#[test]
fn investigation_clue_npc_time_rejects_authority_violation_without_event() {
    let contract =
        DomainAuthorityContract::new_locked("campaign_001", AuthorityMode::AiKp, "ai_kp", 1)
            .unwrap();
    let command =
        CommandEnvelope::governed("npc claim", ActorRole::HumanKeeper, AuthorityMode::AiKp);
    let decision = InvestigationClueNpcTimeDecision::for_track(InvestigationClueNpcTimeTrack::Npc);
    let mut store = EventStore::default();

    let error =
        record_investigation_clue_npc_time_decision(&contract, &mut store, &command, decision)
            .unwrap_err();

    assert_eq!(error, DomainError::AuthorityViolation);
    assert!(store.events().is_empty());
}

#[test]
fn investigation_clue_npc_time_keeps_visibility_and_fact_provenance_on_replay() {
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
        "clue reveal",
        ActorRole::HumanKeeper,
        AuthorityMode::HumanKp,
    );
    command.visibility = Visibility::new(VisibilityLabel::KeeperOnly);
    command.fact_provenance = provenance.clone();
    let decision = InvestigationClueNpcTimeDecision::for_track(InvestigationClueNpcTimeTrack::Clue);
    let mut store = EventStore::default();

    let event =
        record_investigation_clue_npc_time_decision(&contract, &mut store, &command, decision)
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
