
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
#[serde(tag = "operation", rename_all = "snake_case")]
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
    file: File,
}

impl DurableSecretCatalog {
    fn open(path: &Path) -> KernelResult<Self> {
        if !path.is_absolute() || path.file_name().is_none() {
            return Err(TrpgError::InvalidConfiguration(
                "secret_catalog_path_invalid",
            ));
        }
        let parent = path.parent().ok_or(TrpgError::InvalidConfiguration(
            "secret_catalog_path_invalid",
        ))?;
        let parent_metadata = std::fs::symlink_metadata(parent)
            .map_err(|_| TrpgError::InvalidConfiguration("secret_catalog_parent_missing"))?;
        if !parent_metadata.is_dir() || parent_metadata.file_type().is_symlink() {
            return Err(TrpgError::InvalidConfiguration(
                "secret_catalog_parent_invalid",
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if parent_metadata.permissions().mode() & 0o022 != 0 {
                return Err(TrpgError::InvalidConfiguration(
                    "secret_catalog_parent_permissions_too_broad",
                ));
            }
        }
        let file = rustix::fs::open(
            path,
            rustix::fs::OFlags::RDWR
                | rustix::fs::OFlags::CREATE
                | rustix::fs::OFlags::APPEND
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
        )
        .map(File::from)
        .map_err(|_| TrpgError::InvalidConfiguration("secret_catalog_open_failed"))?;
        let metadata = file
            .metadata()
            .map_err(|_| TrpgError::InvalidConfiguration("secret_catalog_open_failed"))?;
        if !metadata.is_file() {
            return Err(TrpgError::InvalidConfiguration(
                "secret_catalog_not_regular_file",
            ));
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
        let mut durable = Self { file };
        durable.load()?;
        Ok(durable)
    }

    fn load(&mut self) -> KernelResult<SecretCatalog> {
        self.with_exclusive_lock(replay_catalog)
    }

    fn apply(&mut self, mutation: &CatalogMutation) -> KernelResult<SecretCatalog> {
        self.with_exclusive_lock(|file| {
            let mut catalog = replay_catalog(file)?;
            mutation.apply(&mut catalog)?;
            file.seek(SeekFrom::End(0))
                .map_err(|_| TrpgError::AuditIntegrityViolation)?;
            serde_json::to_writer(&mut *file, mutation)
                .map_err(|_| TrpgError::AuditIntegrityViolation)?;
            file.write_all(b"\n")
                .and_then(|_| file.sync_all())
                .map_err(|_| TrpgError::AuditIntegrityViolation)?;
            Ok(catalog)
        })
    }

    fn with_exclusive_lock<T>(
        &mut self,
        operation: impl FnOnce(&mut File) -> KernelResult<T>,
    ) -> KernelResult<T> {
        rustix::fs::flock(&self.file, rustix::fs::FlockOperation::LockExclusive)
            .map_err(|_| TrpgError::AuditIntegrityViolation)?;
        let result = operation(&mut self.file);
        let unlock = rustix::fs::flock(&self.file, rustix::fs::FlockOperation::Unlock)
            .map_err(|_| TrpgError::AuditIntegrityViolation);
        match (result, unlock) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(error), _) => Err(error),
            (_, Err(error)) => Err(error),
        }
    }
}

fn replay_catalog(file: &mut File) -> KernelResult<SecretCatalog> {
    file.seek(SeekFrom::Start(0))
        .map_err(|_| TrpgError::AuditIntegrityViolation)?;
    let reader_file = file
        .try_clone()
        .map_err(|_| TrpgError::AuditIntegrityViolation)?;
    let mut catalog = SecretCatalog::default();
    for line in BufReader::new(reader_file).lines() {
        let line = line.map_err(|_| TrpgError::AuditIntegrityViolation)?;
        if line.trim().is_empty() {
            continue;
        }
        let mutation: CatalogMutation =
            serde_json::from_str(&line).map_err(|_| TrpgError::AuditIntegrityViolation)?;
        mutation.apply(&mut catalog)?;
    }
    Ok(catalog)
}

/// Resolves only the currently active version. Rotation immediately revokes
/// the prior version; explicit revocation blocks subsequent leases.
pub struct SecretManager<R> {
    resolver: R,
    catalog: RwLock<SecretCatalog>,
    durable: Option<Mutex<DurableSecretCatalog>>,
}
