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
    integrity_key_id: String,
    integrity_key: [u8; AUDIT_KEY_BYTES],
}

impl std::fmt::Debug for FileAuditLog {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FileAuditLog")
            .field("path", &self.path)
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
        if path.exists() {
            let metadata = path
                .symlink_metadata()
                .map_err(|_| TrpgError::AuditIntegrityViolation)?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(TrpgError::AuditIntegrityViolation);
            }
        }
        let _lock = AuditLock::acquire(&path)?;
        read_and_verify(&path, &integrity_key_id, &key)?;
        Ok(Self {
            path,
            integrity_key_id,
            integrity_key: key,
        })
    }

    pub fn verify(&self) -> KernelResult<Vec<AuditRecord>> {
        let _lock = AuditLock::acquire(&self.path)?;
        read_and_verify(&self.path, &self.integrity_key_id, &self.integrity_key)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl AuditSink for FileAuditLog {
    fn append(&mut self, draft: AuditRecordDraft) -> KernelResult<AuditRecord> {
        let _lock = AuditLock::acquire(&self.path)?;
        let records = read_and_verify(&self.path, &self.integrity_key_id, &self.integrity_key)?;
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
        Ok(record)
    }
}

fn read_and_verify(
    path: &Path,
    expected_key_id: &str,
    integrity_key: &[u8; AUDIT_KEY_BYTES],
) -> KernelResult<Vec<AuditRecord>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let metadata = path
        .symlink_metadata()
        .map_err(|_| TrpgError::AuditIntegrityViolation)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(TrpgError::AuditIntegrityViolation);
    }
    let file = File::open(path).map_err(|_| TrpgError::AuditIntegrityViolation)?;
    let mut records = Vec::new();
    let mut previous_hash = GENESIS_HASH.to_owned();
    for (index, line) in BufReader::new(file).lines().enumerate() {
        let line = line.map_err(|_| TrpgError::AuditIntegrityViolation)?;
        let record: AuditRecord =
            serde_json::from_str(&line).map_err(|_| TrpgError::AuditIntegrityViolation)?;
        validate_fields(&record)?;
        if record.sequence != index as u64 + 1
            || record.integrity_key_id != expected_key_id
            || record.previous_hash != previous_hash
            || record.record_hash != hash_record(&record, integrity_key)?
        {
            return Err(TrpgError::AuditIntegrityViolation);
        }
        previous_hash = record.record_hash.clone();
        records.push(record);
    }
    Ok(records)
}

fn validate_fields(record: &AuditRecord) -> KernelResult<()> {
    let required = [
        record.actor_id.as_str(),
        record.actor_origin.as_str(),
        record.authentication_reference.as_str(),
        record.campaign_id.as_str(),
        record.resource_type.as_str(),
        record.resource_id.as_str(),
        record.action.as_str(),
        record.openfga_decision_id.as_str(),
        record.openfga_policy_revision.as_str(),
        record.opa_decision_id.as_str(),
        record.opa_policy_revision.as_str(),
        record.trace_id.as_str(),
        record.integrity_key_id.as_str(),
        record.previous_hash.as_str(),
    ];
    if record.sequence == 0
        || record.timestamp_unix_ms == 0
        || required.iter().any(|value| value.trim().is_empty())
    {
        return Err(TrpgError::AuditIntegrityViolation);
    }
    Ok(())
}

#[derive(Serialize)]
struct AuditIntegrityPayload<'a> {
    sequence: u64,
    actor_id: &'a str,
    actor_origin: &'a str,
    authentication_reference: &'a str,
    campaign_id: &'a str,
    resource_type: &'a str,
    resource_id: &'a str,
    action: &'a str,
    decision: AuditDecision,
    openfga_decision_id: &'a str,
    openfga_policy_revision: &'a str,
    opa_decision_id: &'a str,
    opa_policy_revision: &'a str,
    timestamp_unix_ms: u64,
    trace_id: &'a str,
    integrity_key_id: &'a str,
    previous_hash: &'a str,
}

fn hash_record(
    record: &AuditRecord,
    integrity_key: &[u8; AUDIT_KEY_BYTES],
) -> KernelResult<String> {
    let payload = serde_json::to_vec(&AuditIntegrityPayload {
        sequence: record.sequence,
        actor_id: &record.actor_id,
        actor_origin: &record.actor_origin,
        authentication_reference: &record.authentication_reference,
        campaign_id: &record.campaign_id,
        resource_type: &record.resource_type,
        resource_id: &record.resource_id,
        action: &record.action,
        decision: record.decision,
        openfga_decision_id: &record.openfga_decision_id,
        openfga_policy_revision: &record.openfga_policy_revision,
        opa_decision_id: &record.opa_decision_id,
        opa_policy_revision: &record.opa_policy_revision,
        timestamp_unix_ms: record.timestamp_unix_ms,
        trace_id: &record.trace_id,
        integrity_key_id: &record.integrity_key_id,
        previous_hash: &record.previous_hash,
    })
    .map_err(|_| TrpgError::AuditIntegrityViolation)?;
    let mut mac = HmacSha256::new_from_slice(integrity_key)
        .map_err(|_| TrpgError::AuditIntegrityViolation)?;
    mac.update(&payload);
    Ok(format!(
        "hmac-sha256:{}",
        hex_encode(&mac.finalize().into_bytes())
    ))
}

struct AuditLock {
    path: PathBuf,
}

impl AuditLock {
    fn acquire(audit_path: &Path) -> KernelResult<Self> {
        let mut lock_name = audit_path.as_os_str().to_os_string();
        lock_name.push(".lock");
        let path = PathBuf::from(lock_name);
        for _ in 0..LOCK_RETRY_COUNT {
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(mut file) => {
                    writeln!(file, "{}", std::process::id())
                        .and_then(|()| file.sync_data())
                        .map_err(|_| TrpgError::AuditIntegrityViolation)?;
                    return Ok(Self { path });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    thread::sleep(Duration::from_millis(5));
                }
                Err(_) => return Err(TrpgError::AuditIntegrityViolation),
            }
        }
        Err(TrpgError::AuditIntegrityViolation)
    }
}

impl Drop for AuditLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn now_unix_ms() -> KernelResult<u64> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| TrpgError::AuditIntegrityViolation)?
        .as_millis();
    u64::try_from(millis).map_err(|_| TrpgError::AuditIntegrityViolation)
}
