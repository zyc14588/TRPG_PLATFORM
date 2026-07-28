
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
