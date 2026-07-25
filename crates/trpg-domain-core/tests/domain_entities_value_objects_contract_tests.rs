mod common;

use trpg_domain_core::ddd::{DomainError, FactSource, ProvenanceKind};
use trpg_domain_core::domain_entities_value_objects::MemoryFact;

#[test]
fn memory_fact_only_confirms_event_backed_sources() {
    let error = common::committed_fact_evidence(
        FactSource::AgentDraft,
        ProvenanceKind::AgentProposal,
        "rejected_agent_draft",
    )
    .unwrap_err();
    assert_eq!(error, DomainError::InvalidConfirmedFactSource);

    let evidence = common::committed_fact_evidence(
        FactSource::GameEvent,
        ProvenanceKind::RulesEngineDecision,
        "fact_event",
    )
    .unwrap();
    let fact = MemoryFact::confirmed("fact_event", &evidence).unwrap();
    assert!(fact.is_confirmed());
}
