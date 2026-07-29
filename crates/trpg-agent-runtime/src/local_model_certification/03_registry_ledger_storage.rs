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

impl LocalModelCertificationAuthority {
    fn with_registry_lock<T>(
        &self,
        operation: impl FnOnce(&mut Option<(u64, String)>) -> AgentResult<T>,
    ) -> AgentResult<T> {
        let mut observed_head = self
            .observed_head
            .lock()
            .map_err(|_| invalid_certification_configuration())?;
        let _file_lock = RegistryFileLock::acquire(&self.lock_file)?;
        operation(&mut observed_head)
    }

    fn read_verified_registry(
        &self,
        observed_head: &mut Option<(u64, String)>,
    ) -> AgentResult<Vec<RegistryRecord>> {
        let records = self.read_registry_records()?;
        let anchor = self.read_anchor()?;
        ensure_registry_anchor_matches(&records, anchor.as_ref())?;
        ensure_registry_not_rolled_back(&records, observed_head.as_ref())?;
        *observed_head = records
            .last()
            .map(|record| (record.sequence, record.record_hash.clone()));
        Ok(records)
    }

    fn read_registry_records(&self) -> AgentResult<Vec<RegistryRecord>> {
        let encoded = read_private_file(&self.registry_path)?;
        let lines = complete_lines(&encoded)?;
        let mut records = Vec::with_capacity(lines.len());
        let mut previous_hash = REGISTRY_GENESIS_HASH.to_owned();
        for (index, line) in lines.into_iter().enumerate() {
            let record: RegistryRecord =
                serde_json::from_str(line).map_err(|_| invalid_certification_configuration())?;
            let expected_sequence = u64::try_from(index)
                .ok()
                .and_then(|value| value.checked_add(1))
                .ok_or_else(invalid_certification_configuration)?;
            self.verify_certificate_signature(&record.certificate)
                .map_err(|_| invalid_certification_configuration())?;
            if record.schema_version != REGISTRY_SCHEMA_VERSION
                || record.sequence != expected_sequence
                || record.previous_hash != previous_hash
                || record.record_hash != self.registry_record_hash(&record)?
            {
                return Err(invalid_certification_configuration());
            }
            previous_hash = record.record_hash.clone();
            records.push(record);
        }
        Ok(records)
    }

    fn read_anchor(&self) -> AgentResult<Option<RegistryHeadAnchor>> {
        if !self.anchor_path.exists() {
            return Ok(None);
        }
        let encoded = read_private_file(&self.anchor_path)?;
        let lines = complete_lines(&encoded)?;
        if lines.len() != 1 {
            return Err(invalid_certification_configuration());
        }
        let anchor: RegistryHeadAnchor = serde_json::from_str(lines[0])
            .map_err(|_| invalid_certification_configuration())?;
        if anchor.schema_version != REGISTRY_SCHEMA_VERSION
            || anchor.sequence == 0
            || anchor.signing_key_id != self.signing_key_id
            || anchor.chain_head.trim().is_empty()
            || anchor.anchor_mac != self.registry_anchor_mac(&anchor)?
        {
            return Err(invalid_certification_configuration());
        }
        Ok(Some(anchor))
    }

    fn write_anchor(&self, record: &RegistryRecord) -> AgentResult<()> {
        let mut anchor = RegistryHeadAnchor {
            schema_version: REGISTRY_SCHEMA_VERSION,
            sequence: record.sequence,
            chain_head: record.record_hash.clone(),
            signing_key_id: self.signing_key_id.clone(),
            anchor_mac: String::new(),
        };
        anchor.anchor_mac = self.registry_anchor_mac(&anchor)?;
        let mut encoded =
            serde_json::to_vec(&anchor).map_err(|_| invalid_certification_configuration())?;
        encoded.push(b'\n');
        write_private_atomic(&self.anchor_path, &encoded)
    }
}

fn ensure_registry_anchor_matches(
    records: &[RegistryRecord],
    anchor: Option<&RegistryHeadAnchor>,
) -> AgentResult<()> {
    match (records.last(), anchor) {
        (None, None) => Ok(()),
        (Some(record), Some(anchor))
            if record.sequence == anchor.sequence && record.record_hash == anchor.chain_head =>
        {
            Ok(())
        }
        _ => Err(invalid_certification_configuration()),
    }
}

fn ensure_registry_not_rolled_back(
    records: &[RegistryRecord],
    observed_head: Option<&(u64, String)>,
) -> AgentResult<()> {
    let Some((sequence, record_hash)) = observed_head else {
        return Ok(());
    };
    let index = usize::try_from(sequence.saturating_sub(1))
        .map_err(|_| invalid_certification_configuration())?;
    if records
        .get(index)
        .is_none_or(|record| record.sequence != *sequence || record.record_hash != *record_hash)
    {
        return Err(invalid_certification_configuration());
    }
    Ok(())
}

struct RegistryFileLock<'a> {
    file: &'a File,
}

impl<'a> RegistryFileLock<'a> {
    fn acquire(file: &'a File) -> AgentResult<Self> {
        file.lock()
            .map_err(|_| invalid_certification_configuration())?;
        Ok(Self { file })
    }
}

impl Drop for RegistryFileLock<'_> {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}
