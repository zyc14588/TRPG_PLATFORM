use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::agent_runtime::{AgentError, AgentResult};
use trpg_security_governance::secret::{
    ledger_checkpoint_id, LedgerCheckpoint, LedgerCheckpointStore,
};

type HmacSha256 = Hmac<Sha256>;
const MAX_CERTIFICATE_TTL: Duration = Duration::from_secs(30 * 24 * 60 * 60);
const REGISTRY_SCHEMA_VERSION: u32 = 1;
const REGISTRY_GENESIS_HASH: &str =
    "hmac-sha256:0000000000000000000000000000000000000000000000000000000000000000";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LocalModelLevel {
    Level0,
    Level1,
    Level2,
    Level3,
    Level4,
}

impl LocalModelLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Level0 => "LOCAL_MODEL_LEVEL_0",
            Self::Level1 => "LOCAL_MODEL_LEVEL_1",
            Self::Level2 => "LOCAL_MODEL_LEVEL_2",
            Self::Level3 => "LOCAL_MODEL_LEVEL_3",
            Self::Level4 => "LOCAL_MODEL_LEVEL_4",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CertificationInput {
    pub model_id: String,
    pub json_schema_support: bool,
    pub tool_call_support: bool,
    pub visibility_tests_pass: bool,
    pub prompt_injection_tests_pass: bool,
    pub rules_eval_pass: bool,
    pub latency_ms: u64,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalModelCertificate {
    certificate_id: String,
    model_id: String,
    model_artifact_sha256: String,
    suite_id: String,
    level: LocalModelLevel,
    issued_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    signing_key_id: String,
    signature: String,
}

impl std::fmt::Debug for LocalModelCertificate {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LocalModelCertificate")
            .field("certificate_id", &self.certificate_id)
            .field("model_id", &self.model_id)
            .field("model_artifact_sha256", &self.model_artifact_sha256)
            .field("suite_id", &self.suite_id)
            .field("level", &self.level)
            .field("issued_at_unix_ms", &self.issued_at_unix_ms)
            .field("expires_at_unix_ms", &self.expires_at_unix_ms)
            .field("signing_key_id", &self.signing_key_id)
            .field("signature", &"[REDACTED]")
            .finish()
    }
}

impl LocalModelCertificate {
    pub fn certificate_id(&self) -> &str {
        &self.certificate_id
    }

    pub fn model_id(&self) -> &str {
        &self.model_id
    }

    pub fn model_artifact_sha256(&self) -> &str {
        &self.model_artifact_sha256
    }

    pub const fn level(&self) -> LocalModelLevel {
        self.level
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
enum RegistryState {
    Active,
    Revoked,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PreviousRegistryEntry {
    certificate: LocalModelCertificate,
    state: RegistryState,
    registry_mac: String,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum RegistryRecordSource {
    Native,
    PreviousFormatMigration,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryRecord {
    schema_version: u32,
    sequence: u64,
    previous_hash: String,
    source: RegistryRecordSource,
    certificate: LocalModelCertificate,
    state: RegistryState,
    record_hash: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryHeadAnchor {
    schema_version: u32,
    sequence: u64,
    chain_head: String,
    signing_key_id: String,
    anchor_mac: String,
}

#[derive(Serialize)]
struct RegistryIntegrityPayload<'a> {
    schema_version: u32,
    sequence: u64,
    previous_hash: &'a str,
    source: RegistryRecordSource,
    certificate: &'a LocalModelCertificate,
    state: RegistryState,
}

#[derive(Serialize)]
struct RegistryAnchorIntegrityPayload<'a> {
    schema_version: u32,
    sequence: u64,
    chain_head: &'a str,
    signing_key_id: &'a str,
}

#[derive(Serialize)]
struct RegistryCheckpointIntegrityPayload<'a> {
    schema_version: u32,
    ledger_id: &'a str,
    sequence: u64,
    previous_chain_head: &'a str,
    chain_head: &'a str,
    signing_key_id: &'a str,
}

/// Signing authority plus append-only durable registry. The HMAC key is
/// zeroized, certificate fields are model/artifact/suite/time bound, and every
/// registry state transition is chained to an independently persisted,
/// authenticated high-water anchor.
pub struct LocalModelCertificationAuthority {
    signing_key_id: String,
    signing_key: Zeroizing<[u8; 32]>,
    registry_path: PathBuf,
    anchor_path: PathBuf,
    ledger_id: String,
    checkpoint_store: Arc<dyn LedgerCheckpointStore>,
    lock_file: File,
    observed_head: Mutex<Option<(u64, String)>>,
}

impl std::fmt::Debug for LocalModelCertificationAuthority {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LocalModelCertificationAuthority")
            .field("signing_key_id", &self.signing_key_id)
            .field("signing_key", &"[REDACTED]")
            .field("registry_path", &self.registry_path)
            .field("anchor_path", &self.anchor_path)
            .field("ledger_id", &self.ledger_id)
            .field("checkpoint_store", &"[EXTERNAL]")
            .finish()
    }
}

fn validate_registry_configuration(signing_key_id: &str, path: &Path) -> AgentResult<()> {
    if signing_key_id.trim().is_empty()
        || signing_key_id.len() > 128
        || !path.is_absolute()
        || path.file_name().is_none()
    {
        return Err(invalid_certification_configuration());
    }
    let parent = path
        .parent()
        .ok_or_else(invalid_certification_configuration)?;
    let metadata =
        fs::symlink_metadata(parent).map_err(|_| invalid_certification_configuration())?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(invalid_certification_configuration());
    }
    Ok(())
}

fn companion_path(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}

fn open_or_create_private_file(path: &Path) -> AgentResult<File> {
    let file = rustix::fs::open(
        path,
        rustix::fs::OFlags::RDWR
            | rustix::fs::OFlags::CREATE
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
    )
    .map(File::from)
    .map_err(|_| invalid_certification_configuration())?;
    validate_private_file(&file)?;
    Ok(file)
}

fn open_private_read(path: &Path) -> AgentResult<File> {
    let file = rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map(File::from)
    .map_err(|_| invalid_certification_configuration())?;
    validate_private_file(&file)?;
    Ok(file)
}

fn open_private_append(path: &Path) -> AgentResult<File> {
    let file = rustix::fs::open(
        path,
        rustix::fs::OFlags::RDWR
            | rustix::fs::OFlags::APPEND
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map(File::from)
    .map_err(|_| invalid_certification_configuration())?;
    validate_private_file(&file)?;
    Ok(file)
}

fn validate_private_file(file: &File) -> AgentResult<()> {
    let metadata = file
        .metadata()
        .map_err(|_| invalid_certification_configuration())?;
    if !metadata.is_file() {
        return Err(invalid_certification_configuration());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(invalid_certification_configuration());
        }
    }
    Ok(())
}

fn validate_private_file_if_present(path: &Path) -> AgentResult<()> {
    if !path.exists() {
        return Ok(());
    }
    let file = open_private_read(path)?;
    validate_private_file(&file)
}

fn read_private_file(path: &Path) -> AgentResult<Vec<u8>> {
    let mut file = open_private_read(path)?;
    let mut encoded = Vec::new();
    file.read_to_end(&mut encoded)
        .map_err(|_| invalid_certification_configuration())?;
    Ok(encoded)
}

fn complete_lines(encoded: &[u8]) -> AgentResult<Vec<&str>> {
    if encoded.is_empty() {
        return Ok(Vec::new());
    }
    if !encoded.ends_with(b"\n") {
        return Err(invalid_certification_configuration());
    }
    let text =
        std::str::from_utf8(encoded).map_err(|_| invalid_certification_configuration())?;
    let lines: Vec<_> = text.strip_suffix('\n').unwrap_or(text).split('\n').collect();
    if lines.iter().any(|line| line.trim().is_empty()) {
        return Err(invalid_certification_configuration());
    }
    Ok(lines)
}

fn encode_registry_records(records: &[RegistryRecord]) -> AgentResult<Vec<u8>> {
    let mut encoded = Vec::new();
    for record in records {
        serde_json::to_writer(&mut encoded, record)
            .map_err(|_| invalid_certification_configuration())?;
        encoded.push(b'\n');
    }
    Ok(encoded)
}

fn write_private_atomic(path: &Path, encoded: &[u8]) -> AgentResult<()> {
    let temporary_path = companion_path(
        path,
        &format!(
            ".tmp-{}-{}",
            std::process::id(),
            trusted_now_unix_ms()?
        ),
    );
    let result = (|| {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temporary_path)
            .map_err(|_| invalid_certification_configuration())?;
        file.write_all(encoded)
            .and_then(|()| file.sync_all())
            .map_err(|_| invalid_certification_configuration())?;
        fs::rename(&temporary_path, path).map_err(|_| invalid_certification_configuration())?;
        sync_parent(path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    result
}

fn sync_parent(path: &Path) -> AgentResult<()> {
    let parent = path
        .parent()
        .ok_or_else(invalid_certification_configuration)?;
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| invalid_certification_configuration())
}

fn sha256_label(encoded: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(encoded))
}

fn valid_sha256_label(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
}
