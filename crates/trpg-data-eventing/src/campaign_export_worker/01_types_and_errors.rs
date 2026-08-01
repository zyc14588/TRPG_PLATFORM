use std::collections::BTreeSet;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use chrono::{DateTime, TimeZone, Utc};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Row, Transaction};

use crate::event_store_sqlx_outbox_projection::{CanonicalReplayEvent, PostgresCanonicalStore};

pub const CAMPAIGN_EXPORT_SCHEMA: &str = "trpg.campaign-export.v1";
pub const VISIBILITY_POLICY_VERSION: &str = "visibility-policy-v1";
const CLAIM_LEASE_SECONDS: i64 = 120;
const MAX_ATTEMPTS: i16 = 5;

#[derive(Clone)]
pub struct CampaignExportWorker {
    pool: PgPool,
    canonical: PostgresCanonicalStore,
    root: PathBuf,
    worker_id: String,
    retention: Duration,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CampaignExportOutcome {
    Idle,
    Completed {
        export_id: String,
    },
    RetryScheduled {
        export_id: String,
        error_code: &'static str,
    },
    TerminalFailure {
        export_id: String,
        error_code: &'static str,
    },
    Expired {
        export_id: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CampaignExportWorkerError {
    code: &'static str,
}

impl CampaignExportWorkerError {
    fn new(code: &'static str) -> Self {
        Self { code }
    }

    pub const fn code(&self) -> &'static str {
        self.code
    }
}

impl fmt::Display for CampaignExportWorkerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code)
    }
}

impl std::error::Error for CampaignExportWorkerError {}

#[derive(Debug)]
struct ClaimedExport {
    export_id: String,
    attempt_count: i16,
}

#[derive(Debug)]
struct ExportRequest {
    export_id: String,
    campaign_id: String,
    requested_by: String,
    audience: String,
    requested_at: DateTime<Utc>,
    requested_event_sequence: i64,
    authority_contract_id: String,
    authority_mode: String,
    authority_owner: String,
    authority_contract_version: i64,
    ruleset_version: String,
    model_route_snapshot: String,
    fork_id: Option<String>,
    parent_campaign_id: Option<String>,
    source_session_id: Option<String>,
    source_snapshot_hash: Option<String>,
    child_snapshot_hash: Option<String>,
}

struct BuiltArtifact {
    bytes: Vec<u8>,
    artifact_hash: String,
    manifest_hash: String,
    first_event_sequence: i64,
    last_event_sequence: i64,
    event_count: i64,
    subjects: BTreeSet<String>,
    artifact_key: String,
    retention_expires_at: DateTime<Utc>,
}
