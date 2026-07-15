crate::define_ops_runbook_module!(
    BackupRestoreRunbookCommand,
    BackupRestoreRunbookService,
    BackupRestoreRunbookRepository,
    BackupRestoreRunbookError,
    append_backup_restore_runbook_event,
    "backup_restore_runbook",
    "OpsBackupRestoreRunbookRecorded",
    crate::OpsRunbookOperation::BackupRestore,
    [
        "backup_manifest",
        "restore_verification",
        "event_store_hash"
    ],
    "runbooks/backup-restore"
);

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PostgresBackupManifest {
    pub backup_id: String,
    pub archive_filename: String,
    pub sha256: String,
    pub byte_length: u64,
    pub source_service: String,
    pub schema_version: String,
    pub pg_dump_version: String,
    pub created_at_unix_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresBackupArtifact {
    pub archive_path: PathBuf,
    pub manifest_path: PathBuf,
    pub manifest: PostgresBackupManifest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PostgresBackupRestoreError {
    Configuration(&'static str),
    OutputAlreadyExists,
    BackupCommandFailed,
    RestoreCommandFailed,
    ManifestInvalid,
    ArchiveHashMismatch,
    UnsafeInPlaceRestore,
    Io(&'static str),
}

impl fmt::Display for PostgresBackupRestoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration(reason) => write!(formatter, "backup configuration: {reason}"),
            Self::OutputAlreadyExists => formatter.write_str("backup output already exists"),
            Self::BackupCommandFailed => formatter.write_str("pg_dump failed"),
            Self::RestoreCommandFailed => formatter.write_str("pg_restore failed"),
            Self::ManifestInvalid => formatter.write_str("backup manifest invalid"),
            Self::ArchiveHashMismatch => formatter.write_str("backup archive hash mismatch"),
            Self::UnsafeInPlaceRestore => formatter.write_str("in-place restore is forbidden"),
            Self::Io(operation) => write!(formatter, "backup I/O failed: {operation}"),
        }
    }
}

impl std::error::Error for PostgresBackupRestoreError {}

#[derive(Clone, Debug)]
pub struct PostgresBackupExecutor {
    pg_dump: PathBuf,
    pg_restore: PathBuf,
    service_file: PathBuf,
    passfile: Option<PathBuf>,
}

impl PostgresBackupExecutor {
    pub fn new(
        pg_dump: impl Into<PathBuf>,
        pg_restore: impl Into<PathBuf>,
        service_file: impl Into<PathBuf>,
        passfile: Option<PathBuf>,
    ) -> Result<Self, PostgresBackupRestoreError> {
        let executor = Self {
            pg_dump: pg_dump.into(),
            pg_restore: pg_restore.into(),
            service_file: service_file.into(),
            passfile,
        };
        for path in [
            executor.pg_dump.as_path(),
            executor.pg_restore.as_path(),
            executor.service_file.as_path(),
        ] {
            validate_regular_absolute_file(path)?;
        }
        if let Some(passfile) = &executor.passfile {
            validate_regular_absolute_file(passfile)?;
        }
        Ok(executor)
    }

    pub fn create_backup(
        &self,
        source_service: &str,
        output_directory: impl AsRef<Path>,
        backup_id: &str,
        schema_version: &str,
    ) -> Result<PostgresBackupArtifact, PostgresBackupRestoreError> {
        validate_service_name(source_service)?;
        validate_backup_id(backup_id)?;
        if schema_version.trim().is_empty() || schema_version.len() > 128 {
            return Err(PostgresBackupRestoreError::Configuration(
                "invalid_schema_version",
            ));
        }
        let output_directory = output_directory.as_ref();
        prepare_output_directory(output_directory)?;
        let archive_filename = format!("{backup_id}.dump");
        let archive_path = output_directory.join(&archive_filename);
        let manifest_path = output_directory.join(format!("{backup_id}.manifest.json"));
        let partial_path =
            output_directory.join(format!(".{backup_id}.partial.{}", std::process::id()));
        if archive_path.exists() || manifest_path.exists() || partial_path.exists() {
            return Err(PostgresBackupRestoreError::OutputAlreadyExists);
        }

        let mut command = self.command(&self.pg_dump, source_service);
        command
            .arg("--format=custom")
            .arg("--no-owner")
            .arg("--no-privileges")
            .arg("--file")
            .arg(&partial_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let status = command
            .status()
            .map_err(|_| PostgresBackupRestoreError::BackupCommandFailed)?;
        if !status.success() {
            let _ = fs::remove_file(&partial_path);
            return Err(PostgresBackupRestoreError::BackupCommandFailed);
        }
        set_private_file_permissions(&partial_path)?;
        fs::rename(&partial_path, &archive_path)
            .map_err(|_| PostgresBackupRestoreError::Io("publish_archive"))?;
        let (sha256, byte_length) = hash_file(&archive_path)?;
        let pg_dump_version = tool_version(&self.pg_dump)?;
        let created_at_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| PostgresBackupRestoreError::Io("system_time"))?
            .as_millis()
            .try_into()
            .map_err(|_| PostgresBackupRestoreError::Io("system_time"))?;
        let manifest = PostgresBackupManifest {
            backup_id: backup_id.to_owned(),
            archive_filename,
            sha256,
            byte_length,
            source_service: source_service.to_owned(),
            schema_version: schema_version.to_owned(),
            pg_dump_version,
            created_at_unix_ms,
        };
        validate_manifest(&manifest)?;
        let encoded = serde_json::to_vec_pretty(&manifest)
            .map_err(|_| PostgresBackupRestoreError::ManifestInvalid)?;
        let mut manifest_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&manifest_path)
            .map_err(|_| PostgresBackupRestoreError::Io("create_manifest"))?;
        manifest_file
            .write_all(&encoded)
            .and_then(|()| manifest_file.sync_all())
            .map_err(|_| PostgresBackupRestoreError::Io("write_manifest"))?;
        set_private_file_permissions(&manifest_path)?;
        sync_directory(output_directory)?;
        Ok(PostgresBackupArtifact {
            archive_path,
            manifest_path,
            manifest,
        })
    }

    pub fn restore_backup(
        &self,
        target_service: &str,
        manifest_path: impl AsRef<Path>,
    ) -> Result<(), PostgresBackupRestoreError> {
        validate_service_name(target_service)?;
        let manifest_path = manifest_path.as_ref();
        validate_regular_absolute_file(manifest_path)?;
        let manifest_bytes =
            fs::read(manifest_path).map_err(|_| PostgresBackupRestoreError::Io("read_manifest"))?;
        let manifest: PostgresBackupManifest = serde_json::from_slice(&manifest_bytes)
            .map_err(|_| PostgresBackupRestoreError::ManifestInvalid)?;
        validate_manifest(&manifest)?;
        if manifest.source_service == target_service {
            return Err(PostgresBackupRestoreError::UnsafeInPlaceRestore);
        }
        let archive_path = manifest_path
            .parent()
            .ok_or(PostgresBackupRestoreError::ManifestInvalid)?
            .join(&manifest.archive_filename);
        validate_regular_absolute_file(&archive_path)?;
        let (actual_hash, actual_length) = hash_file(&archive_path)?;
        if actual_hash != manifest.sha256 || actual_length != manifest.byte_length {
            return Err(PostgresBackupRestoreError::ArchiveHashMismatch);
        }

        let list_status = Command::new(&self.pg_restore)
            .arg("--list")
            .arg(&archive_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|_| PostgresBackupRestoreError::RestoreCommandFailed)?;
        if !list_status.success() {
            return Err(PostgresBackupRestoreError::RestoreCommandFailed);
        }

        let mut command = self.command(&self.pg_restore, target_service);
        command
            .arg("--exit-on-error")
            .arg("--single-transaction")
            .arg("--no-owner")
            .arg("--no-privileges")
            .arg("--dbname")
            .arg(format!("service={target_service}"))
            .arg(&archive_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let status = command
            .status()
            .map_err(|_| PostgresBackupRestoreError::RestoreCommandFailed)?;
        if status.success() {
            Ok(())
        } else {
            Err(PostgresBackupRestoreError::RestoreCommandFailed)
        }
    }

    fn command(&self, executable: &Path, service: &str) -> Command {
        let mut command = Command::new(executable);
        command
            .env_remove("PGPASSWORD")
            .env_remove("DATABASE_URL")
            .env("PGSERVICEFILE", &self.service_file)
            .env("PGSERVICE", service)
            .env("PGCONNECT_TIMEOUT", "5");
        if let Some(passfile) = &self.passfile {
            command.env("PGPASSFILE", passfile);
        } else {
            command.env_remove("PGPASSFILE");
        }
        command
    }
}

fn validate_manifest(manifest: &PostgresBackupManifest) -> Result<(), PostgresBackupRestoreError> {
    validate_backup_id(&manifest.backup_id)?;
    validate_service_name(&manifest.source_service)?;
    if manifest.archive_filename != format!("{}.dump", manifest.backup_id)
        || !valid_sha256(&manifest.sha256)
        || manifest.byte_length == 0
        || manifest.schema_version.trim().is_empty()
        || manifest.pg_dump_version.trim().is_empty()
        || manifest.created_at_unix_ms == 0
    {
        return Err(PostgresBackupRestoreError::ManifestInvalid);
    }
    Ok(())
}

fn validate_service_name(value: &str) -> Result<(), PostgresBackupRestoreError> {
    if value.trim().is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        Err(PostgresBackupRestoreError::Configuration(
            "invalid_libpq_service_name",
        ))
    } else {
        Ok(())
    }
}

fn validate_backup_id(value: &str) -> Result<(), PostgresBackupRestoreError> {
    if value.trim().is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        Err(PostgresBackupRestoreError::Configuration(
            "invalid_backup_id",
        ))
    } else {
        Ok(())
    }
}

fn validate_regular_absolute_file(path: &Path) -> Result<(), PostgresBackupRestoreError> {
    if !path.is_absolute() {
        return Err(PostgresBackupRestoreError::Configuration(
            "absolute_file_path_required",
        ));
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| PostgresBackupRestoreError::Configuration("required_file_missing"))?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(PostgresBackupRestoreError::Configuration(
            "regular_non_symlink_file_required",
        ));
    }
    Ok(())
}

fn prepare_output_directory(path: &Path) -> Result<(), PostgresBackupRestoreError> {
    if !path.is_absolute() {
        return Err(PostgresBackupRestoreError::Configuration(
            "absolute_output_directory_required",
        ));
    }
    fs::create_dir_all(path)
        .map_err(|_| PostgresBackupRestoreError::Io("create_output_directory"))?;
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| PostgresBackupRestoreError::Io("inspect_output_directory"))?;
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return Err(PostgresBackupRestoreError::Configuration(
            "regular_output_directory_required",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|_| PostgresBackupRestoreError::Io("secure_output_directory"))?;
    }
    Ok(())
}

fn set_private_file_permissions(path: &Path) -> Result<(), PostgresBackupRestoreError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .map_err(|_| PostgresBackupRestoreError::Io("secure_backup_file"))?;
    }
    Ok(())
}

fn sync_directory(path: &Path) -> Result<(), PostgresBackupRestoreError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| PostgresBackupRestoreError::Io("sync_output_directory"))
}

fn hash_file(path: &Path) -> Result<(String, u64), PostgresBackupRestoreError> {
    let mut file =
        File::open(path).map_err(|_| PostgresBackupRestoreError::Io("open_archive_for_hash"))?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut length = 0_u64;
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|_| PostgresBackupRestoreError::Io("hash_archive"))?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
        length = length
            .checked_add(read as u64)
            .ok_or(PostgresBackupRestoreError::Io("archive_too_large"))?;
    }
    Ok((format!("sha256:{}", lower_hex(&digest.finalize())), length))
}

fn tool_version(tool: &Path) -> Result<String, PostgresBackupRestoreError> {
    let output = Command::new(tool)
        .arg("--version")
        .output()
        .map_err(|_| PostgresBackupRestoreError::BackupCommandFailed)?;
    if !output.status.success() {
        return Err(PostgresBackupRestoreError::BackupCommandFailed);
    }
    let version = String::from_utf8(output.stdout)
        .map_err(|_| PostgresBackupRestoreError::BackupCommandFailed)?;
    let version = version.trim().to_owned();
    if version.is_empty() {
        Err(PostgresBackupRestoreError::BackupCommandFailed)
    } else {
        Ok(version)
    }
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn lower_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output
}
