
use serde_json::json;
use sqlx::Row;
use trpg_data_eventing::event_store_projections::CanonicalProjectionHasher;
use trpg_data_eventing::outbox_projection_workers::{CheckpointAdvance, PostgresProjectionWorker};

use support::{draft, P04PostgresHarness};
