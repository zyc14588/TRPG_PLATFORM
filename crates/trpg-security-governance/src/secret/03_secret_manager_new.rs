
impl<R: SecretResolver> SecretManager<R> {
    pub fn new(resolver: R) -> Self {
        Self {
            resolver,
            catalog: RwLock::new(SecretCatalog::default()),
            durable: None,
        }
    }

    pub fn new_durable_with_checkpoint(
        resolver: R,
        catalog_path: impl AsRef<Path>,
        integrity_key_id: impl Into<String>,
        integrity_key: &[u8; 32],
        checkpoint_store: Arc<dyn LedgerCheckpointStore>,
    ) -> KernelResult<Self> {
        let mut durable = DurableSecretCatalog::open(
            catalog_path.as_ref(),
            integrity_key_id.into(),
            integrity_key,
            checkpoint_store,
        )?;
        let catalog = durable.load()?;
        Ok(Self {
            resolver,
            catalog: RwLock::new(catalog),
            durable: Some(Mutex::new(durable)),
        })
    }

    pub fn register(&self, reference: &SecretReference) -> KernelResult<()> {
        if self.durable.is_some() {
            let mutation = CatalogMutation::Register {
                backend: reference.backend,
                secret_id: reference.secret_id.clone(),
                version: reference.version,
            };
            return self.apply_durable(&mutation);
        }
        self.catalog
            .write()
            .map_err(|_| TrpgError::AuditIntegrityViolation)?
            .register(reference)
    }

    pub fn rotate(
        &self,
        current: &SecretReference,
        replacement: &SecretReference,
    ) -> KernelResult<()> {
        if self.durable.is_some() {
            let mutation = CatalogMutation::Rotate {
                backend: current.backend,
                secret_id: current.secret_id.clone(),
                current_version: current.version,
                replacement_version: replacement.version,
            };
            return self.apply_durable(&mutation);
        }
        self.catalog
            .write()
            .map_err(|_| TrpgError::AuditIntegrityViolation)?
            .rotate(current, replacement)
    }

    pub fn revoke(&self, reference: &SecretReference) -> KernelResult<()> {
        if self.durable.is_some() {
            let mutation = CatalogMutation::Revoke {
                backend: reference.backend,
                secret_id: reference.secret_id.clone(),
                version: reference.version,
            };
            return self.apply_durable(&mutation);
        }
        self.catalog
            .write()
            .map_err(|_| TrpgError::AuditIntegrityViolation)?
            .revoke(reference)
    }

    pub fn resolve(&self, reference: &SecretReference) -> KernelResult<SecretValue> {
        if let Some(durable) = &self.durable {
            let catalog = durable
                .lock()
                .map_err(|_| TrpgError::AuditIntegrityViolation)?
                .load()?;
            catalog.authorize(reference)?;
            *self
                .catalog
                .write()
                .map_err(|_| TrpgError::AuditIntegrityViolation)? = catalog;
            return self.resolver.resolve(reference);
        }
        self.catalog
            .read()
            .map_err(|_| TrpgError::AuditIntegrityViolation)?
            .authorize(reference)?;
        self.resolver.resolve(reference)
    }

    fn apply_durable(&self, mutation: &CatalogMutation) -> KernelResult<()> {
        let catalog = self
            .durable
            .as_ref()
            .expect("durable mutation requires durable catalog")
            .lock()
            .map_err(|_| TrpgError::AuditIntegrityViolation)?
            .apply(mutation)?;
        *self
            .catalog
            .write()
            .map_err(|_| TrpgError::AuditIntegrityViolation)? = catalog;
        Ok(())
    }

    /// Opens the production catalog against the independent witness database.
    ///
    /// The witness URL and integrity key are resolved directly only to
    /// bootstrap verification of the catalog that authorizes all subsequent
    /// secret leases. Services still register and resolve those references
    /// through this manager before readiness.
    pub fn new_durable(
        resolver: R,
        catalog_path: impl AsRef<Path>,
    ) -> KernelResult<Self> {
        let integrity_key_id = required_bootstrap_environment("TRPG_CANONICAL_HMAC_KEY_ID")?;
        let integrity_reference =
            bootstrap_mounted_reference("TRPG_CANONICAL_HMAC_KEY_SECRET_ID", "TRPG_CANONICAL_HMAC_KEY_SECRET_VERSION")?;
        let witness_reference = bootstrap_mounted_reference(
            "TRPG_WITNESS_DATABASE_URL_SECRET_ID",
            "TRPG_WITNESS_DATABASE_URL_SECRET_VERSION",
        )?;
        let integrity_key = resolver.resolve(&integrity_reference)?.to_key32()?;
        let witness_url = resolver.resolve(&witness_reference)?;
        let checkpoint_store = witness_url.expose_utf8_to(PostgresLedgerCheckpointStore::connect)??;
        integrity_key.expose_to(|key| {
            Self::new_durable_with_checkpoint(
                resolver,
                catalog_path,
                integrity_key_id,
                key,
                Arc::new(checkpoint_store),
            )
        })
    }
}

fn required_bootstrap_environment(name: &str) -> KernelResult<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or(TrpgError::InvalidConfiguration(
            "secret_catalog_bootstrap_environment_missing",
        ))
}

fn bootstrap_mounted_reference(id_name: &str, version_name: &str) -> KernelResult<SecretReference> {
    let secret_id = required_bootstrap_environment(id_name)?;
    let version = required_bootstrap_environment(version_name)?
        .parse::<u64>()
        .ok()
        .filter(|version| *version > 0)
        .ok_or(TrpgError::InvalidConfiguration(
            "secret_catalog_bootstrap_version_invalid",
        ))?;
    SecretReference::mounted(secret_id, version)
}

/// The legacy migration has no integrity key or external witness and fails
/// closed.
#[deprecated(note = "use migrate_previous_secret_catalog_with_checkpoint")]
pub fn migrate_previous_secret_catalog(
    _path: impl AsRef<Path>,
    _expected_records: u64,
    _expected_catalog_sha256: &str,
) -> KernelResult<()> {
    Err(TrpgError::AuditIntegrityViolation)
}

/// Explicitly upgrades the previous bare-mutation JSONL after the operator
/// supplies a trusted record count, digest, key, and independent witness.
pub fn migrate_previous_secret_catalog_with_checkpoint(
    path: impl AsRef<Path>,
    expected_records: u64,
    expected_catalog_sha256: &str,
    integrity_key_id: impl Into<String>,
    integrity_key: &[u8; 32],
    checkpoint_store: Arc<dyn LedgerCheckpointStore>,
) -> KernelResult<()> {
    let path = path.as_ref();
    let integrity_key_id = integrity_key_id.into();
    validate_secret_catalog_path(path)?;
    validate_secret_integrity_key_id(&integrity_key_id)?;
    if expected_records == 0 || !valid_secret_sha256_label(expected_catalog_sha256) {
        return Err(TrpgError::AuditIntegrityViolation);
    }
    validate_secret_file_if_present(path)?;
    if !path.exists() {
        return Err(TrpgError::AuditIntegrityViolation);
    }
    let anchor_path = secret_companion_path(path, ".head");
    if anchor_path.exists() {
        return Err(TrpgError::AuditIntegrityViolation);
    }
    let ledger_id = ledger_checkpoint_id("secret-catalog", path)?;
    if checkpoint_store.latest(&ledger_id)?.is_some() {
        return Err(TrpgError::AuditIntegrityViolation);
    }
    let lock_file = open_or_create_secret_file(&secret_companion_path(path, ".lock"))?;
    let _lock = SecretCatalogFileLock::acquire(&lock_file)?;
    if anchor_path.exists() || checkpoint_store.latest(&ledger_id)?.is_some() {
        return Err(TrpgError::AuditIntegrityViolation);
    }
    let encoded = read_secret_file(path)?;
    if !secret_sha256_label(&encoded).eq_ignore_ascii_case(expected_catalog_sha256) {
        return Err(TrpgError::AuditIntegrityViolation);
    }
    let lines = complete_secret_lines(&encoded)?;
    if u64::try_from(lines.len()).ok() != Some(expected_records) {
        return Err(TrpgError::AuditIntegrityViolation);
    }
    let mut catalog = SecretCatalog::default();
    let mut records = Vec::with_capacity(lines.len());
    let mut previous_hash = SECRET_CATALOG_GENESIS_HASH.to_owned();
    for (index, line) in lines.into_iter().enumerate() {
        let mutation: CatalogMutation =
            serde_json::from_str(line).map_err(|_| TrpgError::AuditIntegrityViolation)?;
        mutation.apply(&mut catalog)?;
        let mut record = SecretCatalogRecord {
            schema_version: SECRET_CATALOG_SCHEMA_VERSION,
            sequence: index as u64 + 1,
            previous_hash,
            source: SecretCatalogRecordSource::PreviousFormatMigration,
            mutation,
            record_hash: String::new(),
        };
        record.record_hash = secret_catalog_record_hash(&record, integrity_key)?;
        previous_hash = record.record_hash.clone();
        records.push(record);
    }
    write_secret_atomic(path, &encode_secret_records(&records)?)?;
    let latest = records
        .last()
        .ok_or(TrpgError::AuditIntegrityViolation)?;
    write_secret_anchor(&anchor_path, latest)?;
    for record in &records {
        write_secret_checkpoint(
            checkpoint_store.as_ref(),
            &ledger_id,
            &integrity_key_id,
            integrity_key,
            record,
        )?;
    }
    Ok(())
}

/// Upgrades the immediately previous SHA-chained catalog and seeds its
/// independent witness after operator attestation of the complete local file.
pub fn migrate_anchored_secret_catalog(
    path: impl AsRef<Path>,
    expected_records: u64,
    expected_catalog_sha256: &str,
    integrity_key_id: impl Into<String>,
    integrity_key: &[u8; 32],
    checkpoint_store: Arc<dyn LedgerCheckpointStore>,
) -> KernelResult<()> {
    let path = path.as_ref();
    let integrity_key_id = integrity_key_id.into();
    validate_secret_catalog_path(path)?;
    validate_secret_integrity_key_id(&integrity_key_id)?;
    if expected_records == 0 || !valid_secret_sha256_label(expected_catalog_sha256) {
        return Err(TrpgError::AuditIntegrityViolation);
    }
    validate_secret_file_if_present(path)?;
    let anchor_path = secret_companion_path(path, ".head");
    validate_secret_file_if_present(&anchor_path)?;
    if !path.exists() || !anchor_path.exists() {
        return Err(TrpgError::AuditIntegrityViolation);
    }
    let ledger_id = ledger_checkpoint_id("secret-catalog", path)?;
    if checkpoint_store.latest(&ledger_id)?.is_some() {
        return Err(TrpgError::AuditIntegrityViolation);
    }
    let lock_file = open_or_create_secret_file(&secret_companion_path(path, ".lock"))?;
    let _lock = SecretCatalogFileLock::acquire(&lock_file)?;
    if checkpoint_store.latest(&ledger_id)?.is_some() {
        return Err(TrpgError::AuditIntegrityViolation);
    }
    let encoded = read_secret_file(path)?;
    if !secret_sha256_label(&encoded).eq_ignore_ascii_case(expected_catalog_sha256) {
        return Err(TrpgError::AuditIntegrityViolation);
    }
    let (_, previous_records) = read_previous_anchored_secret_records(path)?;
    if u64::try_from(previous_records.len()).ok() != Some(expected_records) {
        return Err(TrpgError::AuditIntegrityViolation);
    }
    let anchor = read_secret_anchor(&anchor_path)?.ok_or(TrpgError::AuditIntegrityViolation)?;
    ensure_secret_anchor_matches(&previous_records, Some(&anchor))?;
    let mut catalog = SecretCatalog::default();
    let mut records = Vec::with_capacity(previous_records.len());
    let mut previous_hash = SECRET_CATALOG_GENESIS_HASH.to_owned();
    for (index, previous) in previous_records.into_iter().enumerate() {
        previous.mutation.apply(&mut catalog)?;
        let mut record = SecretCatalogRecord {
            schema_version: SECRET_CATALOG_SCHEMA_VERSION,
            sequence: index as u64 + 1,
            previous_hash,
            source: SecretCatalogRecordSource::PreviousAnchoredFormatMigration,
            mutation: previous.mutation,
            record_hash: String::new(),
        };
        record.record_hash = secret_catalog_record_hash(&record, integrity_key)?;
        previous_hash = record.record_hash.clone();
        records.push(record);
    }
    write_secret_atomic(path, &encode_secret_records(&records)?)?;
    let latest = records
        .last()
        .ok_or(TrpgError::AuditIntegrityViolation)?;
    write_secret_anchor(&anchor_path, latest)?;
    for record in &records {
        write_secret_checkpoint(
            checkpoint_store.as_ref(),
            &ledger_id,
            &integrity_key_id,
            integrity_key,
            record,
        )?;
    }
    Ok(())
}
