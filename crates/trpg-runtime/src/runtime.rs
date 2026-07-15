use crate::runtime_state_machines::{
    commit_decision, replay_visible_runtime_events, EventStore, RuntimeDecision,
    RuntimeEventPayload, RuntimeResult,
};
use trpg_shared_kernel::{AuthorityContract, CommandEnvelope, EventEnvelope, PrincipalScope};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeBoundarySnapshot {
    pub formal_write_path: &'static str,
    pub canon_store: &'static str,
    pub projection_policy: &'static str,
    pub required_command_fields: &'static [&'static str],
}

pub fn runtime_boundary_snapshot() -> RuntimeBoundarySnapshot {
    RuntimeBoundarySnapshot {
        formal_write_path: "Command -> Workflow -> Decision -> Event Store -> Projection",
        canon_store: "Event Store",
        projection_policy:
            "Projection, cache, RAG, summary, and realtime deltas are rebuildable read models",
        required_command_fields: &[
            "idempotency_key",
            "expected_version",
            "actor",
            "authority_mode",
            "visibility",
            "fact_provenance",
            "correlation_id",
            "causation_id",
        ],
    }
}

pub fn commit_runtime_decision(
    store: &mut EventStore<RuntimeEventPayload>,
    contract: &AuthorityContract,
    command: &CommandEnvelope<RuntimeDecision>,
    decision: RuntimeDecision,
) -> RuntimeResult<Vec<EventEnvelope<RuntimeEventPayload>>> {
    commit_decision(store, contract, command, decision)
}

pub fn replay_runtime_for_principal(
    store: &EventStore<RuntimeEventPayload>,
    principal: &PrincipalScope,
) -> Vec<EventEnvelope<RuntimeEventPayload>> {
    replay_visible_runtime_events(store, principal)
}
