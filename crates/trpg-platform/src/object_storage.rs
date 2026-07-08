use crate::readme::{
    append_platform_event, redact_for_observability, PlatformEvent, PlatformEventEnvelope,
    PlatformEventStore,
};
use trpg_shared_kernel::{CommandEnvelope, KernelResult, TrpgError};

pub const OBJECT_STORED_EVENT: &str = "platform.object_storage.object_stored";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoreObject {
    pub object_id: String,
    pub display_name: String,
    pub content_type: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectDescriptor {
    pub object_id: String,
    pub display_name: String,
    pub content_type: String,
}

pub fn public_object_descriptor(command: &CommandEnvelope<StoreObject>) -> ObjectDescriptor {
    ObjectDescriptor {
        object_id: command.payload.object_id.clone(),
        display_name: redact_for_observability(&command.visibility, &command.payload.display_name),
        content_type: command.payload.content_type.clone(),
    }
}

pub fn store_object(
    store: &mut PlatformEventStore,
    command: &CommandEnvelope<StoreObject>,
) -> KernelResult<PlatformEventEnvelope> {
    if command.payload.object_id.trim().is_empty() {
        return Err(TrpgError::InvalidConfiguration("object_id_required"));
    }

    let descriptor = public_object_descriptor(command);
    append_platform_event(
        store,
        command,
        OBJECT_STORED_EVENT,
        PlatformEvent::ObjectStored {
            object_id: descriptor.object_id,
            display_name: descriptor.display_name,
        },
    )
}
