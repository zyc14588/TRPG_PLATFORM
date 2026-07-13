#![forbid(unsafe_code)]

mod error;
mod event;

pub use error::{UnknownWireErrorCode, WireErrorCode};
pub use event::{
    CanonicalEvent, EventDescriptor, UnknownEventName, CANONICAL_EVENT_SCHEMA_ID,
    CANONICAL_EVENT_VERSION,
};
