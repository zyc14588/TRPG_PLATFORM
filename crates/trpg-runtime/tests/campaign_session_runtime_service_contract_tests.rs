use trpg_runtime::campaign_session_runtime_service;
use trpg_runtime::runtime_state_machines::RuntimeEventPayload;
use trpg_runtime::{ActorRole, AuthorityContract, AuthorityMode, CommandEnvelope, EventStore};

fn command(payload: &str, expected_version: u64, idempotency_key: &str) -> CommandEnvelope<String> {
    let mut command = trpg_test_support::governed_command!(
        payload.to_owned(),
        ActorRole::Workflow,
        AuthorityMode::AiKp
    );
    command.expected_version = expected_version;
    command.idempotency_key = idempotency_key.to_owned();
    command
}

#[test]
fn campaign_session_runtime_service_appends_session_and_workflow_events() {
    let contract = AuthorityContract::new("camp_ai_harbor", AuthorityMode::AiKp, 1).unwrap();
    let mut store = EventStore::default();

    let session_event = campaign_session_runtime_service::start_campaign_session(
        &mut store,
        &contract,
        &command("start", 0, "idem_b013_session"),
        "session_013",
    )
    .unwrap();
    let workflow_event = campaign_session_runtime_service::advance_campaign_workflow(
        &mut store,
        &contract,
        &command("advance", 1, "idem_b013_workflow"),
        "workflow_013",
    )
    .unwrap();

    assert_eq!(session_event.event_type, "SessionStarted");
    assert_eq!(workflow_event.event_type, "WorkflowAdvanced");
    assert!(matches!(
        store.events()[0].payload,
        RuntimeEventPayload::SessionStarted { .. }
    ));
    assert!(matches!(
        store.events()[1].payload,
        RuntimeEventPayload::WorkflowAdvanced { .. }
    ));
}

#[test]
fn campaign_session_runtime_service_enforces_expected_version() {
    let contract = AuthorityContract::new("camp_ai_harbor", AuthorityMode::AiKp, 1).unwrap();
    let mut store = EventStore::default();

    let error = campaign_session_runtime_service::start_campaign_session(
        &mut store,
        &contract,
        &command("start", 1, "idem_wrong_version"),
        "session_013",
    )
    .unwrap_err();

    assert_eq!(error.code(), "EXPECTED_VERSION_CONFLICT");
    assert!(store.events().is_empty());
}
