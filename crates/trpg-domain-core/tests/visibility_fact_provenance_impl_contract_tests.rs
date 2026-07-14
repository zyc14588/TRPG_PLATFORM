use trpg_domain_core::ddd::{
    ActorRole, AuthorityMode, DomainError, EntityId, EventStore, FactProvenance, FactSource,
    PrincipalScope, ProvenanceKind, Visibility, VisibilityLabel,
};
use trpg_domain_core::visibility_fact_provenance::{DerivedObject, RedactionOutcome};
use trpg_domain_core::visibility_fact_provenance_impl::{
    append_visibility_fact_decision, confirm_visibility_fact, redact_for_derived_object,
};

#[test]
fn visibility_fact_provenance_impl_rejects_untrusted_confirmed_fact_source() {
    let provenance =
        FactProvenance::new(ProvenanceKind::AgentProposal, "draft_001", "agent_001").unwrap();

    assert_eq!(
        confirm_visibility_fact(
            "fact_001",
            FactSource::AgentDraft,
            Visibility::new(VisibilityLabel::KeeperOnly),
            provenance
        )
        .unwrap_err(),
        DomainError::InvalidConfirmedFactSource
    );
}

#[test]
fn visibility_fact_provenance_impl_preserves_visibility_and_provenance_on_replay() {
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
        "fact promotion",
        ActorRole::HumanKeeper,
    );
    command.visibility = Visibility::new(VisibilityLabel::KeeperOnly);
    command.fact_provenance = provenance.clone();
    let mut store = EventStore::default();

    let event = append_visibility_fact_decision(
        &contract,
        &mut store,
        &command,
        FactSource::DecisionRecord,
    )
    .unwrap();

    assert_eq!(event.fact_provenance, provenance);
    assert_eq!(event.payload.fact_source, FactSource::DecisionRecord);
    assert_eq!(
        redact_for_derived_object(
            &Visibility::new(VisibilityLabel::KeeperOnly),
            DerivedObject::AgentContextForPlayer,
            &PrincipalScope::Player(EntityId::new("player_001").unwrap())
        ),
        RedactionOutcome::Omitted
    );
    assert_eq!(store.replay_visible(&PrincipalScope::Keeper).len(), 1);
    assert!(store
        .replay_visible(&PrincipalScope::Player(
            EntityId::new("player_001").unwrap()
        ))
        .is_empty());
}
