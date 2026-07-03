use trpg_domain_core::ddd::{
    DomainError, FactProvenance, FactSource, PrincipalScope, ProvenanceKind, Visibility,
    VisibilityLabel,
};
use trpg_domain_core::domain_visibility_fact_provenance::{
    confirm_event_sourced_fact, derive_visibility_label, redact_for_derived_object,
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
fn domain_visibility_fact_provenance_rejects_unconfirmed_sources() {
    let provenance =
        FactProvenance::new(ProvenanceKind::AgentProposal, "draft_001", "agent_001").unwrap();

    assert_eq!(
        confirm_event_sourced_fact(
            "fact_001",
            FactSource::AgentDraft,
            Visibility::new(VisibilityLabel::Public),
            provenance
        )
        .unwrap_err(),
        DomainError::InvalidConfirmedFactSource
    );
}

#[test]
fn domain_visibility_fact_provenance_omits_ai_internal_for_player_agent_context() {
    let outcome = redact_for_derived_object(
        &Visibility::new(VisibilityLabel::AiInternal),
        DerivedObject::AgentContextForPlayer,
        &PrincipalScope::PartyMember,
    );

    assert_eq!(outcome, RedactionOutcome::Omitted);
}
