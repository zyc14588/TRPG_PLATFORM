use trpg_platform::background_workers::{
    start_background_worker, StartBackgroundWorker, BACKGROUND_WORKER_STARTED_EVENT,
};
use trpg_platform::{PlatformEvent, PlatformEventStore};
use trpg_shared_kernel::{ActorRole, AuthorityMode, CommandEnvelope, FormalWritePath, TrpgError};

#[test]
fn worker_start_appends_governed_event() {
    let command = CommandEnvelope::governed(
        StartBackgroundWorker {
            worker_id: "worker_001".to_owned(),
            worker_kind: "projection".to_owned(),
        },
        ActorRole::System,
        AuthorityMode::HumanKp,
    );
    let mut store = PlatformEventStore::default();

    let event = start_background_worker(&mut store, &command).expect("worker starts");

    assert_eq!(event.event_type, BACKGROUND_WORKER_STARTED_EVENT);
    assert_eq!(store.events().len(), 1);
    assert!(matches!(
        event.payload,
        PlatformEvent::BackgroundWorkerStarted { .. }
    ));
}

#[test]
fn direct_agent_worker_write_is_rejected() {
    let mut command = CommandEnvelope::governed(
        StartBackgroundWorker {
            worker_id: "worker_001".to_owned(),
            worker_kind: "projection".to_owned(),
        },
        ActorRole::System,
        AuthorityMode::HumanKp,
    );
    command.write_path = FormalWritePath::DirectAgent;
    let mut store = PlatformEventStore::default();

    let err = start_background_worker(&mut store, &command).expect_err("direct agent denied");

    assert_eq!(err, TrpgError::DirectAgentStateWrite);
    assert!(store.events().is_empty());
}
