mod common;

use trpg_domain_core::ddd::{
    DomainError, EntityId, FactSource, PrincipalScope, ProvenanceKind, Visibility, VisibilityLabel,
};
use trpg_domain_core::domain_visibility_fact_provenance::{
    confirm_event_sourced_fact, derive_visibility, derive_visibility_label,
    redact_for_derived_object,
};
use trpg_domain_core::visibility_fact_provenance::{DerivedObject, RedactionOutcome};

#[test]
fn domain_visibility_fact_provenance_uses_most_restrictive_label() {
    assert_eq!(
        derive_visibility_label(&[VisibilityLabel::Public, VisibilityLabel::KeeperOnly]),
        Some(VisibilityLabel::KeeperOnly)
    );
}

#[test]
fn domain_visibility_fact_provenance_preserves_target_scope() {
    let player = EntityId::new("derived_player").unwrap();
    let derived = derive_visibility(&[
        Visibility::new(VisibilityLabel::PartyVisible),
        Visibility::private_to_player(player.clone()),
    ])
    .unwrap()
    .unwrap();
    assert_eq!(derived.subject_id(), Some(&player));
}

#[test]
fn domain_visibility_fact_provenance_rejects_unconfirmed_sources() {
    assert_eq!(
        common::committed_fact_evidence(
            FactSource::AgentDraft,
            ProvenanceKind::AgentProposal,
            "rejected_agent_draft",
        )
        .unwrap_err(),
        DomainError::InvalidConfirmedFactSource
    );

    let evidence = common::committed_fact_evidence(
        FactSource::DecisionRecord,
        ProvenanceKind::RulesEngineDecision,
        "fact_001",
    )
    .unwrap();
    let fact = confirm_event_sourced_fact("fact_001", &evidence).unwrap();
    assert_eq!(fact.source(), FactSource::DecisionRecord);
}

#[test]
fn domain_visibility_fact_provenance_omits_ai_internal_for_player_agent_context() {
    let outcome = redact_for_derived_object(
        &Visibility::new(VisibilityLabel::AiInternal),
        DerivedObject::AgentContextForPlayer,
        &PrincipalScope::System,
        &PrincipalScope::PartyMember,
    );

    assert_eq!(outcome, RedactionOutcome::Omitted);
}
