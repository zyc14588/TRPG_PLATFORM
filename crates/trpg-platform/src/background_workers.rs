use crate::readme::{
    append_platform_event, PlatformEvent, PlatformEventEnvelope, PlatformEventStore,
};
use trpg_shared_kernel::{CommandEnvelope, KernelResult, TrpgError};

pub const BACKGROUND_WORKER_STARTED_EVENT: &str = "platform.background_worker.started";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartBackgroundWorker {
    pub worker_id: String,
    pub worker_kind: String,
}

pub fn start_background_worker(
    store: &mut PlatformEventStore,
    command: &CommandEnvelope<StartBackgroundWorker>,
) -> KernelResult<PlatformEventEnvelope> {
    if command.payload.worker_id.trim().is_empty() {
        return Err(TrpgError::InvalidConfiguration(
            "background_worker_id_required",
        ));
    }

    append_platform_event(
        store,
        command,
        BACKGROUND_WORKER_STARTED_EVENT,
        PlatformEvent::BackgroundWorkerStarted {
            worker_id: command.payload.worker_id.clone(),
            worker_kind: command.payload.worker_kind.clone(),
        },
    )
}
