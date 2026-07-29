const SECRET_CATALOG_SCHEMA_VERSION: u32 = 1;
const SECRET_CATALOG_GENESIS_HASH: &str =
    "sha256:0000000000000000000000000000000000000000000000000000000000000000";

#[derive(Clone, Copy, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum SecretCatalogRecordSource {
    Native,
    PreviousFormatMigration,
}

#[derive(Clone, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct SecretCatalogRecord {
    schema_version: u32,
    sequence: u64,
    previous_hash: String,
    source: SecretCatalogRecordSource,
    mutation: CatalogMutation,
    record_hash: String,
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct SecretCatalogHeadAnchor {
    schema_version: u32,
    sequence: u64,
    chain_head: String,
    anchor_hash: String,
}

#[derive(serde::Serialize)]
struct SecretCatalogIntegrityPayload<'a> {
    schema_version: u32,
    sequence: u64,
    previous_hash: &'a str,
    source: SecretCatalogRecordSource,
    mutation: &'a CatalogMutation,
}

#[derive(serde::Serialize)]
struct SecretCatalogAnchorIntegrityPayload<'a> {
    schema_version: u32,
    sequence: u64,
    chain_head: &'a str,
}

fn read_verified_secret_catalog(
    path: &Path,
    anchor_path: &Path,
    observed_head: &mut Option<(u64, String)>,
) -> KernelResult<(SecretCatalog, Vec<SecretCatalogRecord>)> {
    let (catalog, records) = read_secret_records(path)?;
    let anchor = read_secret_anchor(anchor_path)?;
    ensure_secret_anchor_matches(&records, anchor.as_ref())?;
    ensure_secret_catalog_not_rolled_back(&records, observed_head.as_ref())?;
    *observed_head = records
        .last()
        .map(|record| (record.sequence, record.record_hash.clone()));
    Ok((catalog, records))
}

fn read_secret_records(
    path: &Path,
) -> KernelResult<(SecretCatalog, Vec<SecretCatalogRecord>)> {
    let encoded = read_secret_file(path)?;
    let lines = complete_secret_lines(&encoded)?;
    let mut catalog = SecretCatalog::default();
    let mut records = Vec::with_capacity(lines.len());
    let mut previous_hash = SECRET_CATALOG_GENESIS_HASH.to_owned();
    for (index, line) in lines.into_iter().enumerate() {
        let record: SecretCatalogRecord =
            serde_json::from_str(line).map_err(|_| TrpgError::AuditIntegrityViolation)?;
        let expected_sequence = u64::try_from(index)
            .ok()
            .and_then(|value| value.checked_add(1))
            .ok_or(TrpgError::AuditIntegrityViolation)?;
        if record.schema_version != SECRET_CATALOG_SCHEMA_VERSION
            || record.sequence != expected_sequence
            || record.previous_hash != previous_hash
            || record.record_hash != secret_catalog_record_hash(&record)?
        {
            return Err(TrpgError::AuditIntegrityViolation);
        }
        record.mutation.apply(&mut catalog)?;
        previous_hash = record.record_hash.clone();
        records.push(record);
    }
    Ok((catalog, records))
}

fn read_secret_anchor(path: &Path) -> KernelResult<Option<SecretCatalogHeadAnchor>> {
    if !path.exists() {
        return Ok(None);
    }
    let encoded = read_secret_file(path)?;
    let lines = complete_secret_lines(&encoded)?;
    if lines.len() != 1 {
        return Err(TrpgError::AuditIntegrityViolation);
    }
    let anchor: SecretCatalogHeadAnchor =
        serde_json::from_str(lines[0]).map_err(|_| TrpgError::AuditIntegrityViolation)?;
    if anchor.schema_version != SECRET_CATALOG_SCHEMA_VERSION
        || anchor.sequence == 0
        || anchor.chain_head.trim().is_empty()
        || anchor.anchor_hash != secret_catalog_anchor_hash(&anchor)?
    {
        return Err(TrpgError::AuditIntegrityViolation);
    }
    Ok(Some(anchor))
}

fn write_secret_anchor(path: &Path, record: &SecretCatalogRecord) -> KernelResult<()> {
    let mut anchor = SecretCatalogHeadAnchor {
        schema_version: SECRET_CATALOG_SCHEMA_VERSION,
        sequence: record.sequence,
        chain_head: record.record_hash.clone(),
        anchor_hash: String::new(),
    };
    anchor.anchor_hash = secret_catalog_anchor_hash(&anchor)?;
    let mut encoded =
        serde_json::to_vec(&anchor).map_err(|_| TrpgError::AuditIntegrityViolation)?;
    encoded.push(b'\n');
    write_secret_atomic(path, &encoded)
}

fn secret_catalog_record_hash(record: &SecretCatalogRecord) -> KernelResult<String> {
    let encoded = serde_json::to_vec(&SecretCatalogIntegrityPayload {
        schema_version: record.schema_version,
        sequence: record.sequence,
        previous_hash: &record.previous_hash,
        source: record.source,
        mutation: &record.mutation,
    })
    .map_err(|_| TrpgError::AuditIntegrityViolation)?;
    Ok(secret_sha256_label(&encoded))
}

fn secret_catalog_anchor_hash(anchor: &SecretCatalogHeadAnchor) -> KernelResult<String> {
    let encoded = serde_json::to_vec(&SecretCatalogAnchorIntegrityPayload {
        schema_version: anchor.schema_version,
        sequence: anchor.sequence,
        chain_head: &anchor.chain_head,
    })
    .map_err(|_| TrpgError::AuditIntegrityViolation)?;
    Ok(secret_sha256_label(&encoded))
}

fn ensure_secret_anchor_matches(
    records: &[SecretCatalogRecord],
    anchor: Option<&SecretCatalogHeadAnchor>,
) -> KernelResult<()> {
    match (records.last(), anchor) {
        (None, None) => Ok(()),
        (Some(record), Some(anchor))
            if record.sequence == anchor.sequence && record.record_hash == anchor.chain_head =>
        {
            Ok(())
        }
        _ => Err(TrpgError::AuditIntegrityViolation),
    }
}

fn ensure_secret_catalog_not_rolled_back(
    records: &[SecretCatalogRecord],
    observed_head: Option<&(u64, String)>,
) -> KernelResult<()> {
    let Some((sequence, record_hash)) = observed_head else {
        return Ok(());
    };
    let index =
        usize::try_from(sequence.saturating_sub(1)).map_err(|_| TrpgError::AuditIntegrityViolation)?;
    if records
        .get(index)
        .is_none_or(|record| record.sequence != *sequence || record.record_hash != *record_hash)
    {
        return Err(TrpgError::AuditIntegrityViolation);
    }
    Ok(())
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
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
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
    let lines: Vec<_> = text.strip_suffix('\n').unwrap_or(text).split('\n').collect();
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
