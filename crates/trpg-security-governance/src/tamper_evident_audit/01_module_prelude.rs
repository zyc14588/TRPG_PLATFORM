use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use trpg_shared_kernel::{KernelResult, TrpgError};

type HmacSha256 = Hmac<Sha256>;

const GENESIS_HASH: &str =
    "hmac-sha256:0000000000000000000000000000000000000000000000000000000000000000";
const AUDIT_KEY_BYTES: usize = 32;
const LOCK_RETRY_COUNT: usize = 400;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuditDecision {
    Permit,
    Deny,
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuditRecordDraft {
    pub actor_id: String,
    pub actor_origin: String,
    pub authentication_reference: String,
    pub campaign_id: String,
    pub resource_type: String,
    pub resource_id: String,
    pub action: String,
    pub requested_role: String,
    pub visibility_label: String,
    pub visibility_subject: String,
    pub provenance_kind: String,
    pub provenance_reference: String,
    pub provenance_recorded_by: String,
    pub decision: AuditDecision,
    pub openfga_decision_id: String,
    pub openfga_policy_revision: String,
    pub opa_decision_id: String,
    pub opa_policy_revision: String,
    pub trace_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditRecord {
    pub sequence: u64,
    pub actor_id: String,
    pub actor_origin: String,
    pub authentication_reference: String,
    pub campaign_id: String,
    pub resource_type: String,
    pub resource_id: String,
    pub action: String,
    pub requested_role: String,
    pub visibility_label: String,
    pub visibility_subject: String,
    pub provenance_kind: String,
    pub provenance_reference: String,
    pub provenance_recorded_by: String,
    pub decision: AuditDecision,
    pub openfga_decision_id: String,
    pub openfga_policy_revision: String,
    pub opa_decision_id: String,
    pub opa_policy_revision: String,
    pub timestamp_unix_ms: u64,
    pub trace_id: String,
    pub integrity_key_id: String,
    pub previous_hash: String,
    pub record_hash: String,
}

pub trait AuditSink {
    fn append(&mut self, draft: AuditRecordDraft) -> KernelResult<AuditRecord>;
}

pub struct FileAuditLog {
    path: PathBuf,
    anchor_path: PathBuf,
    integrity_key_id: String,
    integrity_key: [u8; AUDIT_KEY_BYTES],
    observed_head: Option<(u64, String)>,
}

impl std::fmt::Debug for FileAuditLog {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FileAuditLog")
            .field("path", &self.path)
            .field("anchor_path", &self.anchor_path)
            .field("integrity_key_id", &self.integrity_key_id)
            .field("integrity_key", &"[REDACTED]")
            .finish()
    }
}

impl FileAuditLog {
    pub fn open(
        path: impl AsRef<Path>,
        integrity_key_id: impl Into<String>,
        integrity_key: &[u8],
    ) -> KernelResult<Self> {
        let path = path.as_ref().to_path_buf();
        let integrity_key_id = integrity_key_id.into();
        if integrity_key_id.trim().is_empty() || integrity_key.len() != AUDIT_KEY_BYTES {
            return Err(TrpgError::InvalidConfiguration(
                "audit_integrity_key_invalid",
            ));
        }
        let mut key = [0_u8; AUDIT_KEY_BYTES];
        key.copy_from_slice(integrity_key);
        let anchor_path = audit_anchor_path(&path);
        validate_regular_file_if_present(&path)?;
        validate_regular_file_if_present(&anchor_path)?;
        let _lock = AuditLock::acquire(&path)?;
        let records = read_and_verify(&path, &integrity_key_id, &key)?;
        let anchor = read_and_verify_anchor(&anchor_path, &integrity_key_id, &key)?;
        ensure_anchor_matches_latest(&records, anchor.as_ref())?;
        let observed_head = records
            .last()
            .map(|record| (record.sequence, record.record_hash.clone()));
        Ok(Self {
            path,
            anchor_path,
            integrity_key_id,
            integrity_key: key,
            observed_head,
        })
    }

    pub fn verify(&self) -> KernelResult<Vec<AuditRecord>> {
        let _lock = AuditLock::acquire(&self.path)?;
        let records = read_and_verify(&self.path, &self.integrity_key_id, &self.integrity_key)?;
        let anchor = read_and_verify_anchor(
            &self.anchor_path,
            &self.integrity_key_id,
            &self.integrity_key,
        )?;
        ensure_anchor_matches_latest(&records, anchor.as_ref())?;
        ensure_not_rolled_back(&records, self.observed_head.as_ref())?;
        Ok(records)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn anchor_path(&self) -> &Path {
        &self.anchor_path
    }
}

impl AuditSink for FileAuditLog {
    fn append(&mut self, draft: AuditRecordDraft) -> KernelResult<AuditRecord> {
        let _lock = AuditLock::acquire(&self.path)?;
        let records = read_and_verify(&self.path, &self.integrity_key_id, &self.integrity_key)?;
        let anchor = read_and_verify_anchor(
            &self.anchor_path,
            &self.integrity_key_id,
            &self.integrity_key,
        )?;
        ensure_anchor_matches_latest(&records, anchor.as_ref())?;
        ensure_not_rolled_back(&records, self.observed_head.as_ref())?;
        let sequence = records.last().map_or(1, |record| record.sequence + 1);
        let previous_hash = records.last().map_or_else(
            || GENESIS_HASH.to_owned(),
            |record| record.record_hash.clone(),
        );
        let mut record = AuditRecord {
            sequence,
            actor_id: draft.actor_id,
            actor_origin: draft.actor_origin,
            authentication_reference: draft.authentication_reference,
            campaign_id: draft.campaign_id,
            resource_type: draft.resource_type,
            resource_id: draft.resource_id,
            action: draft.action,
            requested_role: draft.requested_role,
            visibility_label: draft.visibility_label,
            visibility_subject: draft.visibility_subject,
            provenance_kind: draft.provenance_kind,
            provenance_reference: draft.provenance_reference,
            provenance_recorded_by: draft.provenance_recorded_by,
            decision: draft.decision,
            openfga_decision_id: draft.openfga_decision_id,
            openfga_policy_revision: draft.openfga_policy_revision,
            opa_decision_id: draft.opa_decision_id,
            opa_policy_revision: draft.opa_policy_revision,
            timestamp_unix_ms: now_unix_ms()?,
            trace_id: draft.trace_id,
            integrity_key_id: self.integrity_key_id.clone(),
            previous_hash,
            record_hash: String::new(),
        };
        validate_fields(&record)?;
        record.record_hash = hash_record(&record, &self.integrity_key)?;
        let encoded =
            serde_json::to_vec(&record).map_err(|_| TrpgError::AuditIntegrityViolation)?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|_| TrpgError::AuditIntegrityViolation)?;
        file.write_all(&encoded)
            .and_then(|()| file.write_all(b"\n"))
            .and_then(|()| file.sync_data())
            .map_err(|_| TrpgError::AuditIntegrityViolation)?;
        write_anchor_atomic(
            &self.anchor_path,
            &AuditHeadAnchor::for_record(&record, &self.integrity_key_id, &self.integrity_key)?,
        )?;
        self.observed_head = Some((record.sequence, record.record_hash.clone()));
        Ok(record)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct AuditHeadAnchor {
    sequence: u64,
    record_hash: String,
    integrity_key_id: String,
    anchor_hash: String,
}

impl AuditHeadAnchor {
    fn for_record(
        record: &AuditRecord,
        integrity_key_id: &str,
        integrity_key: &[u8; AUDIT_KEY_BYTES],
    ) -> KernelResult<Self> {
        let mut anchor = Self {
            sequence: record.sequence,
            record_hash: record.record_hash.clone(),
            integrity_key_id: integrity_key_id.to_owned(),
            anchor_hash: String::new(),
        };
        anchor.anchor_hash = hash_anchor(&anchor, integrity_key)?;
        Ok(anchor)
    }
}

fn ensure_not_rolled_back(
    records: &[AuditRecord],
    observed_head: Option<&(u64, String)>,
) -> KernelResult<()> {
    let Some((sequence, record_hash)) = observed_head else {
        return Ok(());
    };
    let index = usize::try_from(sequence.saturating_sub(1))
        .map_err(|_| TrpgError::AuditIntegrityViolation)?;
    if records
        .get(index)
        .is_none_or(|record| record.sequence != *sequence || record.record_hash != *record_hash)
    {
        return Err(TrpgError::AuditIntegrityViolation);
    }
    Ok(())
}

fn ensure_anchor_matches_latest(
    records: &[AuditRecord],
    anchor: Option<&AuditHeadAnchor>,
) -> KernelResult<()> {
    match (records.last(), anchor) {
        (None, None) => Ok(()),
        (Some(record), Some(anchor))
            if record.sequence == anchor.sequence && record.record_hash == anchor.record_hash =>
        {
            Ok(())
        }
        _ => Err(TrpgError::AuditIntegrityViolation),
    }
}

fn audit_anchor_path(audit_path: &Path) -> PathBuf {
    let mut name = audit_path.as_os_str().to_os_string();
    name.push(".head");
    PathBuf::from(name)
}

fn validate_regular_file_if_present(path: &Path) -> KernelResult<()> {
    if !path.exists() {
        return Ok(());
    }
    let metadata = path
        .symlink_metadata()
        .map_err(|_| TrpgError::AuditIntegrityViolation)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(TrpgError::AuditIntegrityViolation);
    }
    Ok(())
}

fn read_and_verify_anchor(
    path: &Path,
    expected_key_id: &str,
    integrity_key: &[u8; AUDIT_KEY_BYTES],
) -> KernelResult<Option<AuditHeadAnchor>> {
    if !path.exists() {
        return Ok(None);
    }
    validate_regular_file_if_present(path)?;
    let encoded = fs::read_to_string(path).map_err(|_| TrpgError::AuditIntegrityViolation)?;
    let anchor: AuditHeadAnchor =
        serde_json::from_str(encoded.trim()).map_err(|_| TrpgError::AuditIntegrityViolation)?;
    if anchor.sequence == 0
        || anchor.integrity_key_id != expected_key_id
        || anchor.record_hash.trim().is_empty()
        || anchor.anchor_hash != hash_anchor(&anchor, integrity_key)?
    {
        return Err(TrpgError::AuditIntegrityViolation);
    }
    Ok(Some(anchor))
}

fn write_anchor_atomic(path: &Path, anchor: &AuditHeadAnchor) -> KernelResult<()> {
    let mut temporary_name = path.as_os_str().to_os_string();
    temporary_name.push(format!(".tmp-{}-{}", std::process::id(), now_unix_ms()?));
    let temporary_path = PathBuf::from(temporary_name);
    let encoded = serde_json::to_vec(anchor).map_err(|_| TrpgError::AuditIntegrityViolation)?;
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)
            .map_err(|_| TrpgError::AuditIntegrityViolation)?;
        file.write_all(&encoded)
            .and_then(|()| file.write_all(b"\n"))
            .and_then(|()| file.sync_all())
            .map_err(|_| TrpgError::AuditIntegrityViolation)?;
        fs::rename(&temporary_path, path).map_err(|_| TrpgError::AuditIntegrityViolation)?;
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|_| TrpgError::AuditIntegrityViolation)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    result
}
