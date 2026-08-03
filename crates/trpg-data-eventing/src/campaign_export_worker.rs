//! Rebuildable asynchronous campaign-export worker.
//!
//! The canonical request/event range remains in Event Store. This worker owns
//! only leased read-model state and deterministic artifacts; it has no
//! canonical commit capability.

// Decomposed to keep each human-maintained source file within the project limit.
include!("campaign_export_worker/01_types_and_errors.rs");
include!("campaign_export_worker/02_claim_and_event_loading.rs");
include!("campaign_export_worker/03_artifact_persistence.rs");
include!("campaign_export_worker/04_expiration.rs");
include!("campaign_export_worker/05_visibility_manifest_and_storage.rs");
#[cfg(test)]
include!("campaign_export_worker/06_tests.rs");
