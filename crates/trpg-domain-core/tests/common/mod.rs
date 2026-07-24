#![allow(dead_code)]

use trpg_domain_core::command_cqrs::{CommandAcceptedPayload, DomainCommandKind};
use trpg_domain_core::ddd::{
    ActorRole, AuthorityMode, DomainResult, EventStore, FactProvenance, FactSource, ProvenanceKind,
    Visibility, VisibilityLabel,
};
use trpg_domain_core::visibility_fact_provenance::CommittedFactEvidence;

pub fn committed_fact_evidence(
    source: FactSource,
    provenance_kind: ProvenanceKind,
    target_fact_id: &str,
) -> DomainResult<CommittedFactEvidence> {
    let mut command = trpg_test_support::governed_command(
        "formal fact fixture",
        ActorRole::RulesEngine,
        AuthorityMode::HumanKp,
    );
    command.visibility = Visibility::new(VisibilityLabel::Public);
    command.fact_provenance =
        FactProvenance::new(provenance_kind, "formal_source_001", "rules_engine_001")?;
    let mut store = EventStore::default();
    let event = store.append(
        &command,
        trpg_domain_core::visibility_fact_provenance::expected_source_event_type(source),
        CommandAcceptedPayload {
            kind: DomainCommandKind::RecordDecision,
            fact_source: source,
            target_fact_id: target_fact_id.to_owned(),
        },
    )?;
    CommittedFactEvidence::load(&store, event.sequence, target_fact_id)
}
