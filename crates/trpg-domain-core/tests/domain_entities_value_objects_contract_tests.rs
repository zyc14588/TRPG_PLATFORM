use trpg_domain_core::ddd::{
    DomainError, FactProvenance, FactSource, ProvenanceKind, Visibility, VisibilityLabel,
};
use trpg_domain_core::domain_entities_value_objects::MemoryFact;

#[test]
fn memory_fact_only_confirms_event_backed_sources() {
    let provenance = FactProvenance::new(
        ProvenanceKind::RulesEngineDecision,
        "event_001",
        "rules_001",
    )
    .unwrap();

    let error = MemoryFact::confirmed(
        "fact_agent_draft",
        FactSource::AgentDraft,
        Visibility::new(VisibilityLabel::KeeperOnly),
        provenance.clone(),
    )
    .unwrap_err();
    assert_eq!(error, DomainError::InvalidConfirmedFactSource);

    let fact = MemoryFact::confirmed(
        "fact_event",
        FactSource::GameEvent,
        Visibility::new(VisibilityLabel::Public),
        provenance,
    )
    .unwrap();
    assert!(fact.confirmed);
}
