#![forbid(unsafe_code)]

mod error;
mod event;

pub use error::{UnknownWireErrorCode, WireErrorCode};
pub use event::{CanonicalEvent, EventDescriptor, UnknownEventName};
