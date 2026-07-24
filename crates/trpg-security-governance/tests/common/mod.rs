#![allow(dead_code)]

use trpg_domain_core::command_cqrs::{CommandAcceptedPayload, DomainCommandKind};
use trpg_domain_core::ddd::{
    ActorRole, AuthorityMode, EventStore, FactProvenance, FactSource, ProvenanceKind, Visibility,
};
use trpg_domain_core::visibility_fact_provenance::CommittedFactEvidence;
use trpg_security_governance::cloud_egress::CloudContextFact;

pub fn verified_cloud_fact(
    fact_id: &str,
    visibility: Visibility,
    content: Vec<u8>,
) -> CloudContextFact {
    let mut command = trpg_test_support::governed_command(
        "verified cloud-context fixture",
        ActorRole::RulesEngine,
        AuthorityMode::HumanKp,
    );
    command.visibility = visibility;
    command.fact_provenance = FactProvenance::new(
        ProvenanceKind::RulesEngineDecision,
        format!("decision_{fact_id}"),
        "rules_engine_cloud_context",
    )
    .unwrap();
    let mut store = EventStore::default();
    let event = store
        .append(
            &command,
            "DecisionCommitted",
            CommandAcceptedPayload {
                kind: DomainCommandKind::RecordDecision,
                fact_source: FactSource::DecisionRecord,
                target_fact_id: fact_id.to_owned(),
            },
        )
        .unwrap();
    let evidence = CommittedFactEvidence::load(&store, event.sequence, fact_id).unwrap();
    CloudContextFact::from_committed_fact(&evidence, content).unwrap()
}
