use trpg_runtime::runtime_state_machines::RuntimeEventPayload;
use trpg_runtime::saga::{self, SagaCompensationRequest};
use trpg_runtime::{ActorRole, AuthorityContract, AuthorityMode, EventStore, FormalWritePath};

#[test]
fn saga_compensation_uses_governed_event_append() {
    let contract = AuthorityContract::new("camp_ai_harbor", AuthorityMode::AiKp, 1).unwrap();
    let command = trpg_test_support::governed_command!(
        "compensate".to_owned(),
        ActorRole::Workflow,
        AuthorityMode::AiKp,
    );
    let mut store = EventStore::default();

    let event = saga::record_saga_compensation(
        &mut store,
        &contract,
        &command,
        SagaCompensationRequest::new("saga_013").unwrap(),
    )
    .unwrap();

    assert_eq!(event.event_type, "SagaCompensated");
    assert_eq!(event.idempotency_key, command.idempotency_key);
    assert_eq!(event.fact_provenance, command.fact_provenance);
    assert_eq!(event.correlation_id, command.correlation_id);
    assert_eq!(event.causation_id, command.causation_id);
    assert!(matches!(
        event.payload,
        RuntimeEventPayload::SagaCompensated { .. }
    ));
}

#[test]
fn saga_rejects_direct_agent_state_write() {
    let contract = AuthorityContract::new("camp_ai_harbor", AuthorityMode::AiKp, 1).unwrap();
    let mut command = trpg_test_support::governed_command!(
        "compensate".to_owned(),
        ActorRole::Workflow,
        AuthorityMode::AiKp,
    );
    command.write_path = FormalWritePath::DirectAgent;
    let mut store = EventStore::default();

    let error = saga::record_saga_compensation(
        &mut store,
        &contract,
        &command,
        SagaCompensationRequest::new("saga_013").unwrap(),
    )
    .unwrap_err();

    assert_eq!(error.code(), "AGENT_DIRECT_STATE_WRITE_FORBIDDEN");
    assert!(store.events().is_empty());
}
