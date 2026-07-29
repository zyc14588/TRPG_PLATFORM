
impl SecretCatalog {
    fn register(&mut self, reference: &SecretReference) -> KernelResult<()> {
        let identity = (reference.backend, reference.secret_id.clone());
        if self.active.get(&identity) == Some(&reference.version)
            && self.versions.get(&(
                reference.backend,
                reference.secret_id.clone(),
                reference.version,
            )) == Some(&SecretVersionState::Active)
        {
            return Ok(());
        }
        if self.active.contains_key(&identity)
            || self.versions.keys().any(|(backend, secret_id, _)| {
                *backend == reference.backend && secret_id == &reference.secret_id
            })
        {
            return Err(TrpgError::InvalidConfiguration(
                "secret_reference_already_registered",
            ));
        }
        self.versions.insert(
            (
                reference.backend,
                reference.secret_id.clone(),
                reference.version,
            ),
            SecretVersionState::Active,
        );
        self.active.insert(identity, reference.version);
        Ok(())
    }

    fn rotate(
        &mut self,
        current: &SecretReference,
        replacement: &SecretReference,
    ) -> KernelResult<()> {
        if current.backend != replacement.backend
            || current.secret_id != replacement.secret_id
            || replacement.version <= current.version
            || self.versions.contains_key(&(
                replacement.backend,
                replacement.secret_id.clone(),
                replacement.version,
            ))
        {
            return Err(TrpgError::InvalidConfiguration("invalid_secret_rotation"));
        }
        self.authorize(current)?;
        self.versions.insert(
            (current.backend, current.secret_id.clone(), current.version),
            SecretVersionState::Revoked,
        );
        self.versions.insert(
            (
                replacement.backend,
                replacement.secret_id.clone(),
                replacement.version,
            ),
            SecretVersionState::Active,
        );
        self.active.insert(
            (replacement.backend, replacement.secret_id.clone()),
            replacement.version,
        );
        Ok(())
    }

    fn revoke(&mut self, reference: &SecretReference) -> KernelResult<()> {
        self.authorize(reference)?;
        self.versions.insert(
            (
                reference.backend,
                reference.secret_id.clone(),
                reference.version,
            ),
            SecretVersionState::Revoked,
        );
        self.active
            .remove(&(reference.backend, reference.secret_id.clone()));
        Ok(())
    }

    fn authorize(&self, reference: &SecretReference) -> KernelResult<()> {
        let active = self
            .active
            .get(&(reference.backend, reference.secret_id.clone()));
        let state = self.versions.get(&(
            reference.backend,
            reference.secret_id.clone(),
            reference.version,
        ));
        if active == Some(&reference.version) && state == Some(&SecretVersionState::Active) {
            Ok(())
        } else {
            Err(TrpgError::AuthorizationDenied)
        }
    }
}

#[derive(Clone, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields, tag = "operation", rename_all = "snake_case")]
enum CatalogMutation {
    Register {
        backend: SecretBackend,
        secret_id: String,
        version: u64,
    },
    Rotate {
        backend: SecretBackend,
        secret_id: String,
        current_version: u64,
        replacement_version: u64,
    },
    Revoke {
        backend: SecretBackend,
        secret_id: String,
        version: u64,
    },
}

impl CatalogMutation {
    fn apply(&self, catalog: &mut SecretCatalog) -> KernelResult<()> {
        match self {
            Self::Register {
                backend,
                secret_id,
                version,
            } => catalog.register(&SecretReference {
                backend: *backend,
                secret_id: secret_id.clone(),
                version: *version,
            }),
            Self::Rotate {
                backend,
                secret_id,
                current_version,
                replacement_version,
            } => catalog.rotate(
                &SecretReference {
                    backend: *backend,
                    secret_id: secret_id.clone(),
                    version: *current_version,
                },
                &SecretReference {
                    backend: *backend,
                    secret_id: secret_id.clone(),
                    version: *replacement_version,
                },
            ),
            Self::Revoke {
                backend,
                secret_id,
                version,
            } => catalog.revoke(&SecretReference {
                backend: *backend,
                secret_id: secret_id.clone(),
                version: *version,
            }),
        }
    }
}

struct DurableSecretCatalog {
    path: PathBuf,
    anchor_path: PathBuf,
    lock_file: File,
    observed_head: Option<(u64, String)>,
}

impl DurableSecretCatalog {
    fn open(path: &Path) -> KernelResult<Self> {
        validate_secret_catalog_path(path)?;
        let catalog_file = open_or_create_secret_file(path)?;
        catalog_file
            .sync_all()
            .map_err(|_| TrpgError::AuditIntegrityViolation)?;
        sync_secret_parent(path)?;
        let anchor_path = secret_companion_path(path, ".head");
        validate_secret_file_if_present(&anchor_path)?;
        let lock_path = secret_companion_path(path, ".lock");
        let lock_file = open_or_create_secret_file(&lock_path)?;
        let mut durable = Self {
            path: path.to_path_buf(),
            anchor_path,
            lock_file,
            observed_head: None,
        };
        durable.load()?;
        Ok(durable)
    }

    fn load(&mut self) -> KernelResult<SecretCatalog> {
        self.with_exclusive_lock(|path, anchor_path, observed_head| {
            read_verified_secret_catalog(path, anchor_path, observed_head).map(|(catalog, _)| catalog)
        })
    }

    fn apply(&mut self, mutation: &CatalogMutation) -> KernelResult<SecretCatalog> {
        self.with_exclusive_lock(|path, anchor_path, observed_head| {
            let (mut catalog, records) =
                read_verified_secret_catalog(path, anchor_path, observed_head)?;
            mutation.apply(&mut catalog)?;
            let sequence = records.last().map_or(Ok(1), |record| {
                record
                    .sequence
                    .checked_add(1)
                    .ok_or(TrpgError::AuditIntegrityViolation)
            })?;
            let mut record = SecretCatalogRecord {
                schema_version: SECRET_CATALOG_SCHEMA_VERSION,
                sequence,
                previous_hash: records.last().map_or_else(
                    || SECRET_CATALOG_GENESIS_HASH.to_owned(),
                    |record| record.record_hash.clone(),
                ),
                source: SecretCatalogRecordSource::Native,
                mutation: mutation.clone(),
                record_hash: String::new(),
            };
            record.record_hash = secret_catalog_record_hash(&record)?;
            let mut encoded =
                serde_json::to_vec(&record).map_err(|_| TrpgError::AuditIntegrityViolation)?;
            encoded.push(b'\n');
            let mut file = open_secret_append(path)?;
            file.write_all(&encoded)
                .and_then(|()| file.sync_all())
                .map_err(|_| TrpgError::AuditIntegrityViolation)?;
            write_secret_anchor(anchor_path, &record)?;
            *observed_head = Some((record.sequence, record.record_hash));
            Ok(catalog)
        })
    }

    fn with_exclusive_lock<T>(
        &mut self,
        operation: impl FnOnce(
            &Path,
            &Path,
            &mut Option<(u64, String)>,
        ) -> KernelResult<T>,
    ) -> KernelResult<T> {
        let _lock = SecretCatalogFileLock::acquire(&self.lock_file)?;
        operation(&self.path, &self.anchor_path, &mut self.observed_head)
    }
}

/// Explicitly upgrades the previous bare-mutation JSONL after the operator
/// supplies a trusted record count and whole-file digest.
pub fn migrate_previous_secret_catalog(
    path: impl AsRef<Path>,
    expected_records: u64,
    expected_catalog_sha256: &str,
) -> KernelResult<()> {
    let path = path.as_ref();
    validate_secret_catalog_path(path)?;
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
    let lock_path = secret_companion_path(path, ".lock");
    let lock_file = open_or_create_secret_file(&lock_path)?;
    let _lock = SecretCatalogFileLock::acquire(&lock_file)?;
    if anchor_path.exists() {
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
        record.record_hash = secret_catalog_record_hash(&record)?;
        previous_hash = record.record_hash.clone();
        records.push(record);
    }
    write_secret_atomic(path, &encode_secret_records(&records)?)?;
    let latest = records
        .last()
        .ok_or(TrpgError::AuditIntegrityViolation)?;
    write_secret_anchor(&anchor_path, latest)
}

/// Resolves only the currently active version. Rotation immediately revokes
/// the prior version; explicit revocation blocks subsequent leases.
pub struct SecretManager<R> {
    resolver: R,
    catalog: RwLock<SecretCatalog>,
    durable: Option<Mutex<DurableSecretCatalog>>,
}
