// Source is organized into ordered, responsibility-focused sections.
// `include!` preserves this module's privacy boundary and public API.

include!("backup_restore_runbook/01_module_prelude.rs");
include!("backup_restore_runbook/02_set_private_file_permissions.rs");

const MAX_RESTORE_LIST_BYTES: u64 = 16 * 1024 * 1024;

struct TemporaryRestoreList {
    path: PathBuf,
}

impl Drop for TemporaryRestoreList {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn prepare_restore_list(
    pg_restore: &Path,
    archive_path: &Path,
    backup_id: &str,
) -> Result<TemporaryRestoreList, PostgresBackupRestoreError> {
    let parent = archive_path
        .parent()
        .ok_or(PostgresBackupRestoreError::ManifestInvalid)?;
    let path = parent.join(format!(".{backup_id}.restore-list.{}", std::process::id()));
    let output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|_| PostgresBackupRestoreError::Io("create_restore_list"))?;
    let temporary = TemporaryRestoreList { path };
    set_private_file_permissions(&temporary.path)?;
    let status = Command::new(pg_restore)
        .arg("--list")
        .arg(archive_path)
        .stdout(Stdio::from(output))
        .stderr(Stdio::null())
        .status()
        .map_err(|_| PostgresBackupRestoreError::RestoreCommandFailed)?;
    if !status.success() {
        return Err(PostgresBackupRestoreError::RestoreCommandFailed);
    }
    let length = fs::metadata(&temporary.path)
        .map_err(|_| PostgresBackupRestoreError::Io("inspect_restore_list"))?
        .len();
    if length == 0 || length > MAX_RESTORE_LIST_BYTES {
        return Err(PostgresBackupRestoreError::ManifestInvalid);
    }
    let source = fs::read_to_string(&temporary.path)
        .map_err(|_| PostgresBackupRestoreError::ManifestInvalid)?;
    let mut filtered = String::with_capacity(source.len());
    let mut vector_extension_found = false;
    for line in source.lines() {
        match vector_extension_entry(line) {
            Some(true) => vector_extension_found = true,
            Some(false) => {}
            None => {
                filtered.push_str(line);
                filtered.push('\n');
            }
        }
    }
    if !vector_extension_found {
        return Err(PostgresBackupRestoreError::ManifestInvalid);
    }
    let mut output = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(&temporary.path)
        .map_err(|_| PostgresBackupRestoreError::Io("rewrite_restore_list"))?;
    output
        .write_all(filtered.as_bytes())
        .and_then(|()| output.sync_all())
        .map_err(|_| PostgresBackupRestoreError::Io("write_restore_list"))?;
    Ok(temporary)
}

fn vector_extension_entry(line: &str) -> Option<bool> {
    let mut fields = line.split_whitespace();
    let kind = fields.nth(3)?;
    if kind == "EXTENSION"
        && fields.next() == Some("-")
        && fields.next() == Some("vector")
        && fields.next().is_none()
    {
        return Some(true);
    }
    if kind == "COMMENT"
        && fields.next() == Some("-")
        && fields.next() == Some("EXTENSION")
        && fields.next() == Some("vector")
        && fields.next().is_none()
    {
        return Some(false);
    }
    None
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresCheckedRestoreArtifact {
    pub restored_manifest: PostgresBackupManifest,
    pub safety_point: PostgresBackupArtifact,
}

impl PostgresBackupExecutor {
    pub fn reset_restore_target(
        &self,
        psql: impl AsRef<Path>,
        target_service: &str,
    ) -> Result<(), PostgresBackupRestoreError> {
        let psql = psql.as_ref();
        validate_regular_absolute_file(psql)?;
        validate_service_name(target_service)?;
        let mut child = self
            .command(psql, target_service)
            .arg("-X")
            .arg("--set=ON_ERROR_STOP=1")
            .arg("--no-password")
            .arg("--dbname")
            .arg(format!("service={target_service}"))
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| PostgresBackupRestoreError::RestoreCommandFailed)?;
        let mut input = child
            .stdin
            .take()
            .ok_or(PostgresBackupRestoreError::RestoreCommandFailed)?;
        input
            .write_all(b"DROP OWNED BY CURRENT_USER CASCADE;\n")
            .map_err(|_| PostgresBackupRestoreError::RestoreCommandFailed)?;
        drop(input);
        let status = child
            .wait()
            .map_err(|_| PostgresBackupRestoreError::RestoreCommandFailed)?;
        if status.success() {
            Ok(())
        } else {
            Err(PostgresBackupRestoreError::RestoreCommandFailed)
        }
    }

    pub fn restore_backup_filtered(
        &self,
        target_service: &str,
        manifest_path: impl AsRef<Path>,
    ) -> Result<(), PostgresBackupRestoreError> {
        self.restore_backup_filtered_internal(None, target_service, manifest_path.as_ref())
    }

    pub fn restore_backup_filtered_after_reset(
        &self,
        psql: impl AsRef<Path>,
        target_service: &str,
        manifest_path: impl AsRef<Path>,
    ) -> Result<(), PostgresBackupRestoreError> {
        self.restore_backup_filtered_internal(
            Some(psql.as_ref()),
            target_service,
            manifest_path.as_ref(),
        )
    }

    fn restore_backup_filtered_internal(
        &self,
        psql: Option<&Path>,
        target_service: &str,
        manifest_path: &Path,
    ) -> Result<(), PostgresBackupRestoreError> {
        validate_service_name(target_service)?;
        validate_regular_absolute_file(manifest_path)?;
        let manifest_bytes =
            fs::read(manifest_path).map_err(|_| PostgresBackupRestoreError::Io("read_manifest"))?;
        let candidate: PostgresBackupManifest = serde_json::from_slice(&manifest_bytes)
            .map_err(|_| PostgresBackupRestoreError::ManifestInvalid)?;
        validate_manifest(&candidate)?;
        let manifest = self.verify_backup(manifest_path, &candidate.schema_version)?;
        if manifest.source_service == target_service {
            return Err(PostgresBackupRestoreError::UnsafeInPlaceRestore);
        }
        let archive_path = manifest_path
            .parent()
            .ok_or(PostgresBackupRestoreError::ManifestInvalid)?
            .join(&manifest.archive_filename);
        let restore_list =
            prepare_restore_list(&self.pg_restore, &archive_path, &manifest.backup_id)?;
        if let Some(psql) = psql {
            self.reset_restore_target(psql, target_service)?;
        }
        let mut command = self.command(&self.pg_restore, target_service);
        command
            .arg("--exit-on-error")
            .arg("--single-transaction")
            .arg("--clean")
            .arg("--if-exists")
            .arg("--no-owner")
            .arg("--no-privileges")
            .arg("--use-list")
            .arg(&restore_list.path)
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

    pub fn verify_backup(
        &self,
        manifest_path: impl AsRef<Path>,
        expected_schema_version: &str,
    ) -> Result<PostgresBackupManifest, PostgresBackupRestoreError> {
        if expected_schema_version.trim().is_empty() || expected_schema_version.len() > 128 {
            return Err(PostgresBackupRestoreError::Configuration(
                "invalid_expected_schema_version",
            ));
        }
        let manifest_path = manifest_path.as_ref();
        validate_regular_absolute_file(manifest_path)?;
        let manifest_bytes =
            fs::read(manifest_path).map_err(|_| PostgresBackupRestoreError::Io("read_manifest"))?;
        let manifest: PostgresBackupManifest = serde_json::from_slice(&manifest_bytes)
            .map_err(|_| PostgresBackupRestoreError::ManifestInvalid)?;
        validate_manifest(&manifest)?;
        if manifest.schema_version != expected_schema_version {
            return Err(PostgresBackupRestoreError::ManifestInvalid);
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
        Ok(manifest)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn restore_backup_checked(
        &self,
        target_service: &str,
        manifest_path: impl AsRef<Path>,
        expected_schema_version: &str,
        safety_output_directory: impl AsRef<Path>,
        safety_point_id: &str,
    ) -> Result<PostgresCheckedRestoreArtifact, PostgresBackupRestoreError> {
        validate_service_name(target_service)?;
        validate_backup_id(safety_point_id)?;
        let manifest_path = manifest_path.as_ref();
        let restored_manifest = self.verify_backup(manifest_path, expected_schema_version)?;
        if restored_manifest.source_service == target_service {
            return Err(PostgresBackupRestoreError::UnsafeInPlaceRestore);
        }

        // The safety point is completed and durably published before the
        // destructive restore command can start.
        let safety_point = self.create_backup(
            target_service,
            safety_output_directory,
            safety_point_id,
            expected_schema_version,
        )?;
        self.restore_backup_filtered(target_service, manifest_path)?;
        Ok(PostgresCheckedRestoreArtifact {
            restored_manifest,
            safety_point,
        })
    }
}
