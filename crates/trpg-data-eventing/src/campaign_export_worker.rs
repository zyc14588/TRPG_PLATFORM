//! Rebuildable asynchronous campaign-export worker.
//!
//! The canonical request/event range remains in Event Store. This worker owns
//! only leased read-model state and deterministic artifacts; it has no
//! canonical commit capability.

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

impl CampaignExportWorker {
    pub fn new(
        pool: PgPool,
        canonical: PostgresCanonicalStore,
        root: impl Into<PathBuf>,
        worker_id: impl Into<String>,
        retention: Duration,
    ) -> Result<Self, CampaignExportWorkerError> {
        let root = root.into();
        let worker_id = worker_id.into();
        if !safe_id(&worker_id)
            || retention < Duration::from_secs(60)
            || retention > Duration::from_secs(366 * 24 * 60 * 60)
            || root.as_os_str().is_empty()
        {
            return Err(CampaignExportWorkerError::new(
                "CAMPAIGN_EXPORT_CONFIGURATION_INVALID",
            ));
        }
        if fs::symlink_metadata(&root).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
            return Err(CampaignExportWorkerError::new(
                "CAMPAIGN_EXPORT_ROOT_SYMLINK_FORBIDDEN",
            ));
        }
        fs::create_dir_all(&root)
            .map_err(|_| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_ROOT_UNAVAILABLE"))?;
        Ok(Self {
            pool,
            canonical,
            root,
            worker_id,
            retention,
        })
    }

    pub async fn check_readiness(&self) -> Result<(), CampaignExportWorkerError> {
        let ready: bool =
            sqlx::query_scalar("SELECT to_regclass('public.campaign_export_jobs') IS NOT NULL")
                .fetch_one(&self.pool)
                .await
                .map_err(|_| {
                    CampaignExportWorkerError::new("CAMPAIGN_EXPORT_DATABASE_UNAVAILABLE")
                })?;
        if !ready {
            return Err(CampaignExportWorkerError::new(
                "CAMPAIGN_EXPORT_SCHEMA_UNAVAILABLE",
            ));
        }
        let probe = self.root.join(".readiness");
        write_private_file(&probe, b"ready")?;
        fs::remove_file(probe)
            .map_err(|_| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_ROOT_UNAVAILABLE"))
    }

    pub async fn run_once(
        &self,
        now_unix_ms: i64,
    ) -> Result<CampaignExportOutcome, CampaignExportWorkerError> {
        let now = utc_from_unix_ms(now_unix_ms)?;
        if let Some(expired) = self.expire_one(now).await? {
            return Ok(CampaignExportOutcome::Expired { export_id: expired });
        }
        let Some(claim) = self.claim(now).await? else {
            return Ok(CampaignExportOutcome::Idle);
        };
        match self.process_claim(&claim, now).await {
            Ok(()) => Ok(CampaignExportOutcome::Completed {
                export_id: claim.export_id,
            }),
            Err(error) => {
                self.mark_failed(&claim, error.code(), now).await?;
                if claim.attempt_count >= MAX_ATTEMPTS {
                    Ok(CampaignExportOutcome::TerminalFailure {
                        export_id: claim.export_id,
                        error_code: error.code(),
                    })
                } else {
                    Ok(CampaignExportOutcome::RetryScheduled {
                        export_id: claim.export_id,
                        error_code: error.code(),
                    })
                }
            }
        }
    }

    async fn claim(
        &self,
        now: DateTime<Utc>,
    ) -> Result<Option<ClaimedExport>, CampaignExportWorkerError> {
        let row = sqlx::query(
            r#"
            WITH candidate AS (
                SELECT export_id
                  FROM public.campaign_export_jobs
                 WHERE attempt_count < $4
                   AND (
                        state IN ('REQUESTED', 'FAILED') AND next_attempt_at <= $1
                        OR state = 'RUNNING' AND lease_expires_at <= $1
                   )
                 ORDER BY created_at, export_id
                 FOR UPDATE SKIP LOCKED
                 LIMIT 1
            )
            UPDATE public.campaign_export_jobs AS job
               SET state = 'RUNNING',
                   attempt_count = job.attempt_count + 1,
                   lease_owner = $2,
                   lease_expires_at = $1 + make_interval(secs => $3),
                   failure_code = NULL,
                   updated_at = $1
              FROM candidate
             WHERE job.export_id = candidate.export_id
            RETURNING job.export_id, job.attempt_count
            "#,
        )
        .bind(now)
        .bind(&self.worker_id)
        .bind(CLAIM_LEASE_SECONDS)
        .bind(MAX_ATTEMPTS)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_CLAIM_FAILED"))?;
        Ok(row.map(|row| ClaimedExport {
            export_id: row.get("export_id"),
            attempt_count: row.get("attempt_count"),
        }))
    }

    async fn process_claim(
        &self,
        claim: &ClaimedExport,
        _now: DateTime<Utc>,
    ) -> Result<(), CampaignExportWorkerError> {
        let request = self.load_request(&claim.export_id).await?;
        let events = self
            .load_events(&request.campaign_id, request.requested_event_sequence)
            .await?;
        let groups = self.player_groups(&request).await?;
        let subjects = self.campaign_subjects(&request.campaign_id).await?;
        let built = self.build_artifact(&request, &events, &groups, subjects)?;
        self.persist_artifact(claim, &built)?;
        self.mark_ready(claim, built).await
    }

    async fn load_request(
        &self,
        export_id: &str,
    ) -> Result<ExportRequest, CampaignExportWorkerError> {
        let row = sqlx::query(
            r#"
            SELECT export.export_id, export.campaign_id, export.requested_by,
                   export.audience, export.requested_at,
                   job.requested_event_sequence,
                   authority.contract_id AS authority_contract_id,
                   authority.authority_mode, authority.authority_owner,
                   authority.contract_version, authority.ruleset_version,
                   authority.model_route_snapshot,
                   fork.fork_id, fork.parent_campaign_id, fork.source_session_id,
                   fork.source_snapshot_hash, fork.child_snapshot_hash
              FROM public.campaign_exports AS export
              JOIN public.campaign_export_jobs AS job
                ON job.export_id = export.export_id
              JOIN public.authority_contracts AS authority
                ON authority.campaign_id = export.campaign_id
               AND authority.locked
              LEFT JOIN public.campaign_forks AS fork
                ON fork.child_campaign_id = export.campaign_id
             WHERE export.export_id = $1
            "#,
        )
        .bind(export_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_REQUEST_LOAD_FAILED"))?
        .ok_or_else(|| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_REQUEST_MISSING"))?;
        Ok(ExportRequest {
            export_id: row.get("export_id"),
            campaign_id: row.get("campaign_id"),
            requested_by: row.get("requested_by"),
            audience: row.get("audience"),
            requested_at: row.get("requested_at"),
            requested_event_sequence: row.get("requested_event_sequence"),
            authority_contract_id: row.get("authority_contract_id"),
            authority_mode: row.get("authority_mode"),
            authority_owner: row.get("authority_owner"),
            authority_contract_version: row.get("contract_version"),
            ruleset_version: row.get("ruleset_version"),
            model_route_snapshot: row.get("model_route_snapshot"),
            fork_id: row.get("fork_id"),
            parent_campaign_id: row.get("parent_campaign_id"),
            source_session_id: row.get("source_session_id"),
            source_snapshot_hash: row.get("source_snapshot_hash"),
            child_snapshot_hash: row.get("child_snapshot_hash"),
        })
    }

    async fn load_events(
        &self,
        campaign_id: &str,
        through_sequence: i64,
    ) -> Result<Vec<CanonicalReplayEvent>, CampaignExportWorkerError> {
        let mut after = 0_i64;
        let mut events = Vec::new();
        loop {
            let page = self
                .canonical
                .load_replay_page(campaign_id, after, 500)
                .await
                .map_err(|_| {
                    CampaignExportWorkerError::new("CAMPAIGN_EXPORT_CANONICAL_REPLAY_FAILED")
                })?;
            if page.is_empty() {
                break;
            }
            let mut reached_boundary = false;
            for event in page {
                if event.sequence > through_sequence {
                    reached_boundary = true;
                    break;
                }
                after = event.sequence;
                events.push(event);
            }
            if reached_boundary
                || events
                    .last()
                    .is_some_and(|event| event.sequence >= through_sequence)
            {
                break;
            }
        }
        if events.is_empty() || events.last().map(|event| event.sequence) != Some(through_sequence)
        {
            return Err(CampaignExportWorkerError::new(
                "CAMPAIGN_EXPORT_EVENT_RANGE_INCOMPLETE",
            ));
        }
        Ok(events)
    }

    async fn player_groups(
        &self,
        request: &ExportRequest,
    ) -> Result<BTreeSet<String>, CampaignExportWorkerError> {
        if request.audience != "PLAYER" {
            return Ok(BTreeSet::new());
        }
        let rows = sqlx::query(
            r#"
            SELECT group_id
              FROM public.campaign_group_memberships
             WHERE campaign_id = $1 AND user_id = $2 AND revoked_at IS NULL
            "#,
        )
        .bind(&request.campaign_id)
        .bind(&request.requested_by)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_GROUPS_LOAD_FAILED"))?;
        Ok(rows.into_iter().map(|row| row.get("group_id")).collect())
    }

    async fn campaign_subjects(
        &self,
        campaign_id: &str,
    ) -> Result<BTreeSet<String>, CampaignExportWorkerError> {
        let rows =
            sqlx::query("SELECT user_id FROM public.campaign_memberships WHERE campaign_id = $1")
                .bind(campaign_id)
                .fetch_all(&self.pool)
                .await
                .map_err(|_| {
                    CampaignExportWorkerError::new("CAMPAIGN_EXPORT_SUBJECTS_LOAD_FAILED")
                })?;
        Ok(rows.into_iter().map(|row| row.get("user_id")).collect())
    }

    fn build_artifact(
        &self,
        request: &ExportRequest,
        events: &[CanonicalReplayEvent],
        groups: &BTreeSet<String>,
        mut subjects: BTreeSet<String>,
    ) -> Result<BuiltArtifact, CampaignExportWorkerError> {
        subjects.insert(request.requested_by.clone());
        let first_event_sequence = events[0].sequence;
        let last_event_sequence = events[events.len() - 1].sequence;
        let event_count = i64::try_from(events.len())
            .map_err(|_| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_EVENT_COUNT_OVERFLOW"))?;
        let view = normalized_view(&request.audience)?;
        let records = events
            .iter()
            .filter_map(|event| export_record(view, request, groups, event))
            .collect::<Vec<_>>();
        let sections = export_sections(view, &records, request);
        let content = json!({
            "records": records,
            "sections": sections,
        });
        let content_bytes = serde_json::to_vec(&content)
            .map_err(|_| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_SERIALIZATION_FAILED"))?;
        let manifest_hash = sha256_prefixed(&content_bytes);
        let retention_expires_at = request.requested_at
            + chrono::Duration::from_std(self.retention)
                .map_err(|_| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_RETENTION_INVALID"))?;
        let artifact = json!({
            "content": content,
            "manifest": {
                "artifact_schema": CAMPAIGN_EXPORT_SCHEMA,
                "authority": {
                    "contract_id": request.authority_contract_id,
                    "contract_version": request.authority_contract_version,
                    "mode": request.authority_mode,
                    "owner": request.authority_owner,
                    "ruleset_version": request.ruleset_version,
                },
                "campaign_id": request.campaign_id,
                "event_range": {
                    "count": event_count,
                    "first_sequence": first_event_sequence,
                    "last_sequence": last_event_sequence,
                },
                "export_id": request.export_id,
                "fork_provenance": {
                    "child_snapshot_hash": request.child_snapshot_hash,
                    "fork_id": request.fork_id,
                    "parent_campaign_id": request.parent_campaign_id,
                    "source_session_id": request.source_session_id,
                    "source_snapshot_hash": request.source_snapshot_hash,
                },
                "generated_from_request_at": request.requested_at.to_rfc3339(),
                "hash_algorithm": "sha256",
                "manifest_hash": manifest_hash,
                "manifest_hash_scope": "canonical-json:content",
                "requested_by": request.requested_by,
                "retention_expires_at": retention_expires_at.to_rfc3339(),
                "view": view,
                "visibility_policy_version": VISIBILITY_POLICY_VERSION,
            },
        });
        let bytes = serde_json::to_vec(&artifact)
            .map_err(|_| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_SERIALIZATION_FAILED"))?;
        let artifact_hash = sha256_prefixed(&bytes);
        let artifact_key = format!(
            "artifacts/{}/{}.json",
            request.campaign_id, request.export_id
        );
        if !safe_relative_key(&artifact_key) {
            return Err(CampaignExportWorkerError::new(
                "CAMPAIGN_EXPORT_ARTIFACT_KEY_INVALID",
            ));
        }
        Ok(BuiltArtifact {
            bytes,
            artifact_hash,
            manifest_hash,
            first_event_sequence,
            last_event_sequence,
            event_count,
            subjects,
            artifact_key,
            retention_expires_at,
        })
    }

    fn persist_artifact(
        &self,
        claim: &ClaimedExport,
        built: &BuiltArtifact,
    ) -> Result<(), CampaignExportWorkerError> {
        let destination = checked_artifact_path(&self.root, &built.artifact_key)?;
        let parent = destination.parent().ok_or_else(|| {
            CampaignExportWorkerError::new("CAMPAIGN_EXPORT_ARTIFACT_KEY_INVALID")
        })?;
        fs::create_dir_all(parent)
            .map_err(|_| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_WRITE_FAILED"))?;
        if destination.exists() {
            let existing = fs::read(&destination)
                .map_err(|_| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_READ_FAILED"))?;
            if sha256_prefixed(&existing) == built.artifact_hash {
                return Ok(());
            }
            return Err(CampaignExportWorkerError::new(
                "CAMPAIGN_EXPORT_ARTIFACT_HASH_CONFLICT",
            ));
        }
        let temporary = destination.with_extension(format!(
            "json.tmp.{}.{}",
            self.worker_id, claim.attempt_count
        ));
        write_private_file(&temporary, &built.bytes)?;
        fs::rename(&temporary, &destination).map_err(|_| {
            let _ = fs::remove_file(&temporary);
            CampaignExportWorkerError::new("CAMPAIGN_EXPORT_ATOMIC_RENAME_FAILED")
        })
    }

    async fn mark_ready(
        &self,
        claim: &ClaimedExport,
        built: BuiltArtifact,
    ) -> Result<(), CampaignExportWorkerError> {
        let artifact_size = i64::try_from(built.bytes.len()).map_err(|_| {
            CampaignExportWorkerError::new("CAMPAIGN_EXPORT_ARTIFACT_SIZE_OVERFLOW")
        })?;
        let mut transaction =
            self.pool.begin().await.map_err(|_| {
                CampaignExportWorkerError::new("CAMPAIGN_EXPORT_READY_BEGIN_FAILED")
            })?;
        sqlx::query("DELETE FROM public.campaign_export_subjects WHERE export_id = $1")
            .bind(&claim.export_id)
            .execute(&mut *transaction)
            .await
            .map_err(|_| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_SUBJECT_RESET_FAILED"))?;
        for subject in built.subjects {
            sqlx::query(
                "INSERT INTO public.campaign_export_subjects (export_id, subject_id) \
                 VALUES ($1, $2) ON CONFLICT DO NOTHING",
            )
            .bind(&claim.export_id)
            .bind(subject)
            .execute(&mut *transaction)
            .await
            .map_err(|_| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_SUBJECT_BIND_FAILED"))?;
        }
        let updated = sqlx::query(
            r#"
            UPDATE public.campaign_export_jobs
               SET state = 'READY', lease_owner = NULL, lease_expires_at = NULL,
                   first_event_sequence = $3, last_event_sequence = $4,
                   event_count = $5,
                   visibility_policy_version = $6, artifact_schema = $7,
                   artifact_key = $8, artifact_hash = $9, manifest_hash = $10,
                   artifact_size = $11, ready_at = now(),
                   retention_expires_at = $12, failure_code = NULL,
                   updated_at = now()
             WHERE export_id = $1 AND state = 'RUNNING' AND lease_owner = $2
            "#,
        )
        .bind(&claim.export_id)
        .bind(&self.worker_id)
        .bind(built.first_event_sequence)
        .bind(built.last_event_sequence)
        .bind(built.event_count)
        .bind(VISIBILITY_POLICY_VERSION)
        .bind(CAMPAIGN_EXPORT_SCHEMA)
        .bind(built.artifact_key)
        .bind(built.artifact_hash)
        .bind(built.manifest_hash)
        .bind(artifact_size)
        .bind(built.retention_expires_at)
        .execute(&mut *transaction)
        .await
        .map_err(|_| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_READY_UPDATE_FAILED"))?;
        if updated.rows_affected() != 1 {
            return Err(CampaignExportWorkerError::new("CAMPAIGN_EXPORT_LEASE_LOST"));
        }
        transaction
            .commit()
            .await
            .map_err(|_| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_READY_COMMIT_FAILED"))
    }

    async fn mark_failed(
        &self,
        claim: &ClaimedExport,
        error_code: &'static str,
        now: DateTime<Utc>,
    ) -> Result<(), CampaignExportWorkerError> {
        let retry_seconds = i64::from(claim.attempt_count).saturating_mul(2).max(1);
        sqlx::query(
            r#"
            UPDATE public.campaign_export_jobs
               SET state = 'FAILED', lease_owner = NULL, lease_expires_at = NULL,
                   failure_code = $3,
                   next_attempt_at = $4 + make_interval(secs => $5),
                   updated_at = $4
             WHERE export_id = $1 AND state = 'RUNNING' AND lease_owner = $2
            "#,
        )
        .bind(&claim.export_id)
        .bind(&self.worker_id)
        .bind(error_code)
        .bind(now)
        .bind(retry_seconds)
        .execute(&self.pool)
        .await
        .map_err(|_| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_FAILURE_UPDATE_FAILED"))?;
        Ok(())
    }

    async fn expire_one(
        &self,
        now: DateTime<Utc>,
    ) -> Result<Option<String>, CampaignExportWorkerError> {
        let row = sqlx::query(
            r#"
            SELECT export_id, artifact_key
              FROM public.campaign_export_jobs
             WHERE state = 'READY' AND retention_expires_at <= $1
             ORDER BY retention_expires_at, export_id
             LIMIT 1
            "#,
        )
        .bind(now)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_EXPIRY_LOAD_FAILED"))?;
        let Some(row) = row else { return Ok(None) };
        let export_id: String = row.get("export_id");
        let key: String = row.get("artifact_key");
        remove_artifact(&self.root, &key)?;
        let mut transaction: Transaction<'_, Postgres> =
            self.pool.begin().await.map_err(|_| {
                CampaignExportWorkerError::new("CAMPAIGN_EXPORT_EXPIRY_BEGIN_FAILED")
            })?;
        sqlx::query("DELETE FROM public.campaign_export_download_tickets WHERE export_id = $1")
            .bind(&export_id)
            .execute(&mut *transaction)
            .await
            .map_err(|_| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_TICKET_DELETE_FAILED"))?;
        sqlx::query(
            r#"
            UPDATE public.campaign_export_jobs
               SET state = 'EXPIRED', artifact_key = NULL, deleted_at = $2,
                   lease_owner = NULL, lease_expires_at = NULL, updated_at = $2
             WHERE export_id = $1 AND state = 'READY'
            "#,
        )
        .bind(&export_id)
        .bind(now)
        .execute(&mut *transaction)
        .await
        .map_err(|_| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_EXPIRY_UPDATE_FAILED"))?;
        transaction
            .commit()
            .await
            .map_err(|_| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_EXPIRY_COMMIT_FAILED"))?;
        Ok(Some(export_id))
    }
}

fn normalized_view(audience: &str) -> Result<&'static str, CampaignExportWorkerError> {
    match audience {
        "PLAYER" => Ok("PLAYER"),
        "KEEPER_PRIVATE" | "CAMPAIGN_ARCHIVE" => Ok("KEEPER_PRIVATE"),
        "AUDIT" => Ok("AUDIT"),
        _ => Err(CampaignExportWorkerError::new(
            "CAMPAIGN_EXPORT_AUDIENCE_INVALID",
        )),
    }
}

fn export_record(
    view: &str,
    request: &ExportRequest,
    groups: &BTreeSet<String>,
    event: &CanonicalReplayEvent,
) -> Option<Value> {
    let include = match view {
        "PLAYER" => match event.visibility_label.as_str() {
            "public" | "party_visible" | "spectator_visible" | "spectator_hidden" => true,
            "private_to_player" | "investigator_private" => {
                event.visibility_subject == request.requested_by
            }
            "private_to_group" => groups.contains(&event.visibility_subject),
            _ => false,
        },
        "KEEPER_PRIVATE" => !matches!(
            event.visibility_label.as_str(),
            "ai_internal" | "system_only" | "system_private"
        ),
        "AUDIT" => true,
        _ => false,
    };
    if !include {
        return None;
    }
    let payload_hash = serde_json::to_vec(&event.payload)
        .ok()
        .map(|bytes| sha256_prefixed(&bytes))?;
    let restricted_audit_payload = view == "AUDIT"
        && matches!(
            event.visibility_label.as_str(),
            "private_to_player"
                | "investigator_private"
                | "private_to_group"
                | "keeper_only"
                | "ai_internal"
                | "system_only"
                | "system_private"
        );
    let payload = if restricted_audit_payload {
        json!({"payload_hash": payload_hash, "redacted": true})
    } else {
        event.payload.clone()
    };
    Some(json!({
        "event_integrity_hash": event.event_integrity_hash,
        "event_schema_version": event.event_schema_version,
        "event_type": event.event_type,
        "payload": payload,
        "payload_hash": payload_hash,
        "provenance": {
            "kind": event.provenance_kind,
            "recorded_by": event.provenance_recorded_by,
            "reference": event.provenance_reference,
        },
        "recorded_at": event.recorded_at.to_rfc3339(),
        "request_hash": event.request_hash,
        "resource": {"id": event.resource_id, "type": event.resource_type},
        "sequence": event.sequence,
        "stream": {"id": event.stream_id, "version": event.stream_version},
        "visibility": {"label": event.visibility_label, "subject": event.visibility_subject},
    }))
}

fn export_sections(view: &str, records: &[Value], request: &ExportRequest) -> Value {
    let event_refs = |needle: &str| {
        records
            .iter()
            .filter(|record| {
                record["event_type"]
                    .as_str()
                    .is_some_and(|event_type| event_type.contains(needle))
            })
            .map(|record| record["sequence"].clone())
            .collect::<Vec<_>>()
    };
    match view {
        "PLAYER" => json!({
            "discovered_clues": event_refs("Clue"),
            "public_scene_summary": event_refs("Scene"),
            "visible_dice_rolls": event_refs("Dice"),
        }),
        "KEEPER_PRIVATE" => json!({
            "all_public_events": records.iter().filter(|record| {
                matches!(record["visibility"]["label"].as_str(), Some("public" | "party_visible"))
            }).map(|record| record["sequence"].clone()).collect::<Vec<_>>(),
            "hidden_clues": event_refs("Clue"),
            "keeper_truth": records.iter().filter(|record| {
                record["visibility"]["label"] == "keeper_only"
            }).map(|record| record["sequence"].clone()).collect::<Vec<_>>(),
            "npc_secrets": event_refs("Npc"),
        }),
        "AUDIT" => json!({
            "decision_records": event_refs("Decision"),
            "dice_rolls": event_refs("Dice"),
            "model_route_snapshot": request.model_route_snapshot,
            "tool_calls": event_refs("Tool"),
            "visibility_labels": records.iter().filter_map(|record| {
                record["visibility"]["label"].as_str().map(str::to_owned)
            }).collect::<BTreeSet<_>>(),
        }),
        _ => Value::Null,
    }
}

fn utc_from_unix_ms(value: i64) -> Result<DateTime<Utc>, CampaignExportWorkerError> {
    Utc.timestamp_millis_opt(value)
        .single()
        .ok_or_else(|| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_TIME_INVALID"))
}

pub fn artifact_sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn sha256_prefixed(bytes: &[u8]) -> String {
    artifact_sha256(bytes)
}

fn safe_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

pub fn safe_relative_key(key: &str) -> bool {
    let path = Path::new(key);
    !path.is_absolute()
        && !key.is_empty()
        && key.len() <= 512
        && path.components().all(|component| {
            matches!(component, Component::Normal(_))
                && component.as_os_str().to_str().is_some_and(|part| {
                    !part.is_empty()
                        && part.bytes().all(|byte| {
                            byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.')
                        })
                })
        })
}

pub fn checked_artifact_path(root: &Path, key: &str) -> Result<PathBuf, CampaignExportWorkerError> {
    if !safe_relative_key(key) {
        return Err(CampaignExportWorkerError::new(
            "CAMPAIGN_EXPORT_ARTIFACT_KEY_INVALID",
        ));
    }
    Ok(root.join(key))
}

fn write_private_file(path: &Path, bytes: &[u8]) -> Result<(), CampaignExportWorkerError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options
        .open(path)
        .map_err(|_| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_WRITE_FAILED"))?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_WRITE_FAILED"))
}

pub fn remove_artifact(root: &Path, key: &str) -> Result<(), CampaignExportWorkerError> {
    let path = checked_artifact_path(root, key)?;
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(CampaignExportWorkerError::new(
            "CAMPAIGN_EXPORT_DELETE_FAILED",
        )),
    }
}
