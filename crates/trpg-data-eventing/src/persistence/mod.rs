//! PostgreSQL persistence records and replay schema evolution.
//!
//! These types mirror the canonical tables without collapsing JSON, time, or
//! stream metadata into lossy strings.  Database constraints remain the final
//! integrity boundary; the DTOs provide a checked SQLx/Serde mapping.

mod dto;
mod upcaster;

pub use dto::{
    EventOutboxRecord, EventStoreRecord, FormalCommitRecord, ProjectionCheckpointRecord,
};
pub use upcaster::{
    EventPayloadUpcaster, EventUpcastError, UpcastedEventPayload, CURRENT_EVENT_SCHEMA_VERSION,
};
