use trpg_domain_core::ddd::{
    DomainError, EntityId, FactProvenance, FactSource, PrincipalScope, ProvenanceKind, Visibility,
    VisibilityLabel,
};
use trpg_domain_core::visibility_fact_provenance::{
    most_restrictive_label, promote_fact_to_confirmed, redaction_for, DerivedObject,
    RedactionOutcome,
};

#[test]
fn visibility_fact_provenance_redacts_restricted_content() {
    assert_eq!(
        most_restrictive_label(&[VisibilityLabel::Public, VisibilityLabel::KeeperOnly]),
        Some(VisibilityLabel::KeeperOnly)
    );

    assert_eq!(
        redaction_for(
            &Visibility::new(VisibilityLabel::KeeperOnly),
            DerivedObject::AgentContextForPlayer,
            &PrincipalScope::Player(EntityId::new("user_player_a").unwrap())
        ),
        RedactionOutcome::Omitted
    );

    assert_eq!(
        redaction_for(
            &Visibility::new(VisibilityLabel::AiInternal),
            DerivedObject::AnyPlayerOrKeeperExport,
            &PrincipalScope::Keeper
        ),
        RedactionOutcome::RedactedOrAuditOnly
    );
}

#[test]
fn visibility_fact_provenance_blocks_untrusted_confirmed_fact_sources() {
    let provenance = FactProvenance::new(
        ProvenanceKind::RulesEngineDecision,
        "event_001",
        "rules_001",
    )
    .unwrap();

    assert_eq!(
        promote_fact_to_confirmed(
            "fact_npc_claim",
            FactSource::NpcClaim,
            Visibility::new(VisibilityLabel::PartyVisible),
            provenance.clone()
        )
        .unwrap_err(),
        DomainError::InvalidConfirmedFactSource
    );

    let confirmed = promote_fact_to_confirmed(
        "fact_decision",
        FactSource::DecisionRecord,
        Visibility::new(VisibilityLabel::PartyVisible),
        provenance,
    )
    .unwrap();
    assert_eq!(confirmed.source, FactSource::DecisionRecord);
}
