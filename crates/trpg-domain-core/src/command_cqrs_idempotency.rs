use crate::ddd::{
    CommandEnvelope, DomainError, DomainResult, EventEnvelope, EventStore, TrpgError,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdempotencyCheck {
    pub idempotency_key: String,
    pub expected_version: u64,
}

pub fn check_command_metadata<T>(command: &CommandEnvelope<T>) -> DomainResult<IdempotencyCheck> {
    if command.idempotency_key.trim().is_empty() {
        return Err(DomainError::MissingCommandMetadata);
    }

    Ok(IdempotencyCheck {
        idempotency_key: command.idempotency_key.clone(),
        expected_version: command.expected_version,
    })
}

pub fn append_idempotent_event<T, P: Clone>(
    store: &mut EventStore<P>,
    command: &CommandEnvelope<T>,
    event_type: &'static str,
    payload: P,
) -> DomainResult<EventEnvelope<P>> {
    check_command_metadata(command)?;
    store
        .append(command, event_type, payload)
        .map_err(map_event_store_error)
}

fn map_event_store_error(error: TrpgError) -> DomainError {
    DomainError::from(error)
}
