pub fn ledger_checkpoint_id(domain: &str, path: &Path) -> KernelResult<String> {
    if domain.len() < 3
        || domain.len() > 64
        || !domain.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_lowercase() || (index > 0 && (byte.is_ascii_digit() || byte == b'-'))
        })
        || !path.is_absolute()
    {
        return Err(TrpgError::InvalidConfiguration(
            "ledger_checkpoint_identity_invalid",
        ));
    }
    let parent = path.parent().ok_or(TrpgError::InvalidConfiguration(
        "ledger_checkpoint_identity_invalid",
    ))?;
    let metadata = fs::symlink_metadata(parent)
        .map_err(|_| TrpgError::InvalidConfiguration("ledger_checkpoint_identity_invalid"))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(TrpgError::InvalidConfiguration(
            "ledger_checkpoint_identity_invalid",
        ));
    }
    let executable = std::env::current_exe()
        .map_err(|_| TrpgError::InvalidConfiguration("ledger_checkpoint_identity_invalid"))?;
    let executable_name = executable
        .file_name()
        .ok_or(TrpgError::InvalidConfiguration(
            "ledger_checkpoint_identity_invalid",
        ))?;
    let mut digest = Sha256::new();
    digest.update((domain.len() as u64).to_be_bytes());
    digest.update(domain.as_bytes());
    digest.update((path.as_os_str().as_encoded_bytes().len() as u64).to_be_bytes());
    digest.update(path.as_os_str().as_encoded_bytes());
    digest.update((executable_name.as_encoded_bytes().len() as u64).to_be_bytes());
    digest.update(executable_name.as_encoded_bytes());
    Ok(format!("{domain}:sha256:{:x}", digest.finalize()))
}

fn valid_hmac_sha256_label(value: &str) -> bool {
    value.len() == 76
        && value.starts_with("hmac-sha256:")
        && value[12..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

impl PostgresLedgerCheckpointStore {
    pub fn connect(database_url: &str) -> KernelResult<Self> {
        let options = PgConnectOptions::from_str(database_url)
            .map_err(|_| TrpgError::InvalidConfiguration("ledger_checkpoint_database_invalid"))?;
        let host = options.get_host();
        let local = matches!(host, "localhost" | "127.0.0.1" | "::1") || host.starts_with('/');
        if !local && !matches!(options.get_ssl_mode(), PgSslMode::VerifyFull) {
            return Err(TrpgError::InvalidConfiguration(
                "ledger_checkpoint_database_tls_required",
            ));
        }
        Ok(Self { options })
    }
}

impl LedgerCheckpointStore for PostgresLedgerCheckpointStore {
    fn latest(&self, ledger_id: &str) -> KernelResult<Option<LedgerCheckpoint>> {
        let ledger_id = ledger_id.to_owned();
        let options = self.options.clone();
        std::thread::Builder::new()
            .name("ledger-checkpoint-read".to_owned())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|_| TrpgError::AuditIntegrityViolation)?;
                runtime.block_on(async move {
                    let mut connection = PgConnection::connect_with(&options)
                        .await
                        .map_err(|_| TrpgError::AuditIntegrityViolation)?;
                    let row = sqlx::query(
                        "SELECT sequence, previous_chain_head, chain_head, integrity_key_id, \
                         checkpoint_mac FROM public.latest_security_ledger_checkpoint($1)",
                    )
                    .bind(&ledger_id)
                    .fetch_optional(&mut connection)
                    .await
                    .map_err(|_| TrpgError::AuditIntegrityViolation)?;
                    let Some(row) = row else {
                        return Ok(None);
                    };
                    LedgerCheckpoint::new(
                        u64::try_from(
                            row.try_get::<i64, _>("sequence")
                                .map_err(|_| TrpgError::AuditIntegrityViolation)?,
                        )
                        .map_err(|_| TrpgError::AuditIntegrityViolation)?,
                        row.try_get::<String, _>("previous_chain_head")
                            .map_err(|_| TrpgError::AuditIntegrityViolation)?,
                        row.try_get::<String, _>("chain_head")
                            .map_err(|_| TrpgError::AuditIntegrityViolation)?,
                        row.try_get::<String, _>("integrity_key_id")
                            .map_err(|_| TrpgError::AuditIntegrityViolation)?,
                        row.try_get::<String, _>("checkpoint_mac")
                            .map_err(|_| TrpgError::AuditIntegrityViolation)?,
                    )
                    .map(Some)
                })
            })
            .map_err(|_| TrpgError::AuditIntegrityViolation)?
            .join()
            .map_err(|_| TrpgError::AuditIntegrityViolation)?
    }

    fn append(&self, ledger_id: &str, checkpoint: &LedgerCheckpoint) -> KernelResult<()> {
        let ledger_id = ledger_id.to_owned();
        let checkpoint = checkpoint.clone();
        let options = self.options.clone();
        std::thread::Builder::new()
            .name("ledger-checkpoint-append".to_owned())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|_| TrpgError::AuditIntegrityViolation)?;
                runtime.block_on(async move {
                    let mut connection = PgConnection::connect_with(&options)
                        .await
                        .map_err(|_| TrpgError::AuditIntegrityViolation)?;
                    sqlx::query(
                        "SELECT public.append_security_ledger_checkpoint($1, $2, $3, $4, $5, $6)",
                    )
                    .bind(&ledger_id)
                    .bind(
                        i64::try_from(checkpoint.sequence())
                            .map_err(|_| TrpgError::AuditIntegrityViolation)?,
                    )
                    .bind(checkpoint.previous_chain_head())
                    .bind(checkpoint.chain_head())
                    .bind(checkpoint.integrity_key_id())
                    .bind(checkpoint.checkpoint_mac())
                    .execute(&mut connection)
                    .await
                    .map_err(|_| TrpgError::AuditIntegrityViolation)?;
                    Ok(())
                })
            })
            .map_err(|_| TrpgError::AuditIntegrityViolation)?
            .join()
            .map_err(|_| TrpgError::AuditIntegrityViolation)?
    }
}

fn validate_secret_catalog_path(path: &Path) -> KernelResult<()> {
    if !path.is_absolute() || path.file_name().is_none() {
        return Err(TrpgError::InvalidConfiguration(
            "secret_catalog_path_invalid",
        ));
    }
    let parent = path.parent().ok_or(TrpgError::InvalidConfiguration(
        "secret_catalog_path_invalid",
    ))?;
    let metadata = fs::symlink_metadata(parent)
        .map_err(|_| TrpgError::InvalidConfiguration("secret_catalog_parent_missing"))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(TrpgError::InvalidConfiguration(
            "secret_catalog_parent_invalid",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o022 != 0 {
            return Err(TrpgError::InvalidConfiguration(
                "secret_catalog_parent_permissions_too_broad",
            ));
        }
    }
    Ok(())
}

fn secret_companion_path(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}

fn open_or_create_secret_file(path: &Path) -> KernelResult<File> {
    let file = rustix::fs::open(
        path,
        rustix::fs::OFlags::RDWR
            | rustix::fs::OFlags::CREATE
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
    )
    .map(File::from)
    .map_err(|_| TrpgError::AuditIntegrityViolation)?;
    validate_secret_file(&file)?;
    Ok(file)
}

fn open_secret_read(path: &Path) -> KernelResult<File> {
    let file = rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map(File::from)
    .map_err(|_| TrpgError::AuditIntegrityViolation)?;
    validate_secret_file(&file)?;
    Ok(file)
}

fn open_secret_append(path: &Path) -> KernelResult<File> {
    let file = rustix::fs::open(
        path,
        rustix::fs::OFlags::RDWR
            | rustix::fs::OFlags::APPEND
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map(File::from)
    .map_err(|_| TrpgError::AuditIntegrityViolation)?;
    validate_secret_file(&file)?;
    Ok(file)
}

fn validate_secret_file(file: &File) -> KernelResult<()> {
    let metadata = file
        .metadata()
        .map_err(|_| TrpgError::AuditIntegrityViolation)?;
    if !metadata.is_file() {
        return Err(TrpgError::AuditIntegrityViolation);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(TrpgError::InvalidConfiguration(
                "secret_catalog_permissions_too_broad",
            ));
        }
    }
    Ok(())
}

fn validate_secret_file_if_present(path: &Path) -> KernelResult<()> {
    if path.exists() {
        validate_secret_file(&open_secret_read(path)?)?;
    }
    Ok(())
}

fn read_secret_file(path: &Path) -> KernelResult<Vec<u8>> {
    let mut file = open_secret_read(path)?;
    let mut encoded = Vec::new();
    file.read_to_end(&mut encoded)
        .map_err(|_| TrpgError::AuditIntegrityViolation)?;
    Ok(encoded)
}

fn complete_secret_lines(encoded: &[u8]) -> KernelResult<Vec<&str>> {
    if encoded.is_empty() {
        return Ok(Vec::new());
    }
    if !encoded.ends_with(b"\n") {
        return Err(TrpgError::AuditIntegrityViolation);
    }
    let text = std::str::from_utf8(encoded).map_err(|_| TrpgError::AuditIntegrityViolation)?;
    let lines: Vec<_> = text
        .strip_suffix('\n')
        .unwrap_or(text)
        .split('\n')
        .collect();
    if lines.iter().any(|line| line.trim().is_empty()) {
        return Err(TrpgError::AuditIntegrityViolation);
    }
    Ok(lines)
}

fn encode_secret_records(records: &[SecretCatalogRecord]) -> KernelResult<Vec<u8>> {
    let mut encoded = Vec::new();
    for record in records {
        serde_json::to_writer(&mut encoded, record)
            .map_err(|_| TrpgError::AuditIntegrityViolation)?;
        encoded.push(b'\n');
    }
    Ok(encoded)
}

fn write_secret_atomic(path: &Path, encoded: &[u8]) -> KernelResult<()> {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| TrpgError::AuditIntegrityViolation)?
        .as_nanos();
    let temporary_path =
        secret_companion_path(path, &format!(".tmp-{}-{nonce}", std::process::id()));
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
            .map_err(|_| TrpgError::AuditIntegrityViolation)?;
        file.write_all(encoded)
            .and_then(|()| file.sync_all())
            .map_err(|_| TrpgError::AuditIntegrityViolation)?;
        fs::rename(&temporary_path, path).map_err(|_| TrpgError::AuditIntegrityViolation)?;
        sync_secret_parent(path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    result
}

fn sync_secret_parent(path: &Path) -> KernelResult<()> {
    let parent = path.parent().ok_or(TrpgError::AuditIntegrityViolation)?;
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| TrpgError::AuditIntegrityViolation)
}

fn secret_sha256_label(encoded: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(encoded))
}

fn valid_secret_sha256_label(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn secret_hmac_sha256_label(key: &[u8], encoded: &[u8]) -> KernelResult<String> {
    let mut mac =
        HmacSha256::new_from_slice(key).map_err(|_| TrpgError::AuditIntegrityViolation)?;
    mac.update(encoded);
    Ok(format!("hmac-sha256:{:x}", mac.finalize().into_bytes()))
}

fn validate_secret_integrity_key_id(value: &str) -> KernelResult<()> {
    if value.trim().is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        return Err(TrpgError::InvalidConfiguration(
            "secret_catalog_integrity_key_id_invalid",
        ));
    }
    Ok(())
}

struct SecretCatalogFileLock<'a> {
    file: &'a File,
}

impl<'a> SecretCatalogFileLock<'a> {
    fn acquire(file: &'a File) -> KernelResult<Self> {
        file.lock()
            .map_err(|_| TrpgError::AuditIntegrityViolation)?;
        Ok(Self { file })
    }
}

impl Drop for SecretCatalogFileLock<'_> {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}
