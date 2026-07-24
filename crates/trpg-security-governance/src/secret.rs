use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::{Mutex, RwLock};

use trpg_shared_kernel::{KernelResult, TrpgError};
use zeroize::Zeroizing;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
pub enum SecretBackend {
    Kms,
    MountedFile,
    DevelopmentMemory,
}

/// A locator and version, never secret material. Debug output intentionally
/// hides the locator because names can still disclose provider/account data.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct SecretReference {
    backend: SecretBackend,
    secret_id: String,
    version: u64,
}

impl SecretReference {
    pub fn new(
        backend: SecretBackend,
        secret_id: impl Into<String>,
        version: u64,
    ) -> KernelResult<Self> {
        let secret_id = secret_id.into();
        if secret_id.is_empty()
            || secret_id.len() > 256
            || version == 0
            || !secret_id
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
        {
            return Err(TrpgError::InvalidConfiguration("invalid_secret_reference"));
        }
        Ok(Self {
            backend,
            secret_id,
            version,
        })
    }

    pub fn kms(secret_id: impl Into<String>, version: u64) -> KernelResult<Self> {
        Self::new(SecretBackend::Kms, secret_id, version)
    }

    pub fn mounted(secret_id: impl Into<String>, version: u64) -> KernelResult<Self> {
        Self::new(SecretBackend::MountedFile, secret_id, version)
    }

    pub fn development(secret_id: impl Into<String>, version: u64) -> KernelResult<Self> {
        Self::new(SecretBackend::DevelopmentMemory, secret_id, version)
    }

    pub const fn backend(&self) -> SecretBackend {
        self.backend
    }

    pub fn secret_id(&self) -> &str {
        &self.secret_id
    }

    pub const fn version(&self) -> u64 {
        self.version
    }

    pub const fn production_eligible(&self) -> bool {
        matches!(
            self.backend,
            SecretBackend::Kms | SecretBackend::MountedFile
        ) && self.version > 0
    }
}

impl fmt::Debug for SecretReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecretReference")
            .field("backend", &self.backend)
            .field("secret_id", &"[redacted]")
            .field("version", &self.version)
            .finish()
    }
}

/// Secret bytes are zeroized on drop and deliberately implement neither
/// `Clone` nor `Debug`.
///
/// ```compile_fail
/// # use trpg_security_governance::secret::SecretValue;
/// # fn value() -> SecretValue { unreachable!() }
/// println!("{:?}", value());
/// ```
pub struct SecretValue(Zeroizing<Vec<u8>>);

/// Fixed-size cryptographic key material. It is zeroized and deliberately
/// implements neither `Clone` nor `Debug`.
pub struct SecretKey32(Zeroizing<[u8; 32]>);

impl SecretValue {
    fn new(bytes: Vec<u8>) -> KernelResult<Self> {
        if bytes.is_empty() || bytes.len() > 65_536 {
            return Err(TrpgError::InvalidConfiguration("invalid_secret_material"));
        }
        Ok(Self(Zeroizing::new(bytes)))
    }

    /// Exposes bytes only for the duration of a provider-adapter closure.
    pub fn expose_to<R>(&self, consumer: impl FnOnce(&[u8]) -> R) -> R {
        consumer(self.0.as_slice())
    }

    pub fn expose_utf8_to<R>(&self, consumer: impl FnOnce(&str) -> R) -> KernelResult<R> {
        let value = std::str::from_utf8(self.0.as_slice())
            .map_err(|_| TrpgError::InvalidConfiguration("secret_must_be_utf8"))?;
        if value.trim().is_empty() {
            return Err(TrpgError::InvalidConfiguration("secret_must_be_nonblank"));
        }
        Ok(consumer(value))
    }

    pub fn to_key32(&self) -> KernelResult<SecretKey32> {
        let mut key = Zeroizing::new([0_u8; 32]);
        if self.0.len() == 32 {
            key.copy_from_slice(self.0.as_slice());
        } else if self.0.len() == 64 && self.0.iter().all(u8::is_ascii_hexdigit) {
            for (index, pair) in self.0.chunks_exact(2).enumerate() {
                key[index] = (hex_nibble(pair[0])? << 4) | hex_nibble(pair[1])?;
            }
        } else {
            return Err(TrpgError::InvalidConfiguration(
                "secret_key_must_be_32_bytes",
            ));
        }
        Ok(SecretKey32(key))
    }
}

impl SecretKey32 {
    pub fn expose_to<R>(&self, consumer: impl FnOnce(&[u8; 32]) -> R) -> R {
        consumer(&self.0)
    }
}

fn hex_nibble(value: u8) -> KernelResult<u8> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err(TrpgError::InvalidConfiguration("secret_key_hex_invalid")),
    }
}

pub trait SecretResolver: Send + Sync {
    fn resolve(&self, reference: &SecretReference) -> KernelResult<SecretValue>;
}

pub struct MountedFileSecretResolver {
    root_directory: File,
}

impl MountedFileSecretResolver {
    pub fn new(root: impl AsRef<Path>) -> KernelResult<Self> {
        let root = root.as_ref();
        if !root.is_absolute() {
            return Err(TrpgError::InvalidConfiguration(
                "secret_mount_must_be_absolute",
            ));
        }
        let metadata = std::fs::symlink_metadata(root)
            .map_err(|_| TrpgError::InvalidConfiguration("secret_mount_missing"))?;
        if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
            return Err(TrpgError::InvalidConfiguration(
                "secret_mount_must_be_regular_directory",
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o022 != 0 {
                return Err(TrpgError::InvalidConfiguration(
                    "secret_mount_directory_writable_by_untrusted_principal",
                ));
            }
        }
        let root_directory = rustix::fs::open(
            root,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map(File::from)
        .map_err(|_| TrpgError::InvalidConfiguration("secret_mount_open_failed"))?;
        Ok(Self { root_directory })
    }
}

impl SecretResolver for MountedFileSecretResolver {
    fn resolve(&self, reference: &SecretReference) -> KernelResult<SecretValue> {
        if reference.backend != SecretBackend::MountedFile {
            return Err(TrpgError::InvalidConfiguration("secret_backend_mismatch"));
        }
        let file_name = format!("{}.v{}", reference.secret_id, reference.version);
        let mut file = rustix::fs::openat(
            &self.root_directory,
            file_name.as_str(),
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC
                | rustix::fs::OFlags::NONBLOCK,
            rustix::fs::Mode::empty(),
        )
        .map(File::from)
        .map_err(|_| TrpgError::InvalidConfiguration("secret_resolution_failed"))?;
        let metadata = file
            .metadata()
            .map_err(|_| TrpgError::InvalidConfiguration("secret_resolution_failed"))?;
        if !metadata.file_type().is_file() {
            return Err(TrpgError::InvalidConfiguration(
                "secret_mount_not_regular_file",
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o077 != 0 {
                return Err(TrpgError::InvalidConfiguration(
                    "secret_mount_permissions_too_broad",
                ));
            }
        }
        let mut bytes = Vec::new();
        std::io::Read::by_ref(&mut file)
            .take(65_537)
            .read_to_end(&mut bytes)
            .map_err(|_| TrpgError::InvalidConfiguration("secret_resolution_failed"))?;
        while matches!(bytes.last(), Some(b'\n' | b'\r')) {
            bytes.pop();
        }
        SecretValue::new(bytes)
    }
}

pub trait KmsClient: Send + Sync {
    fn decrypt_secret(&self, secret_id: &str, version: u64) -> KernelResult<Vec<u8>>;
}

pub struct KmsSecretResolver<C> {
    client: C,
}

impl<C> KmsSecretResolver<C> {
    pub fn new(client: C) -> Self {
        Self { client }
    }
}

impl<C: KmsClient> SecretResolver for KmsSecretResolver<C> {
    fn resolve(&self, reference: &SecretReference) -> KernelResult<SecretValue> {
        if reference.backend != SecretBackend::Kms {
            return Err(TrpgError::InvalidConfiguration("secret_backend_mismatch"));
        }
        SecretValue::new(
            self.client
                .decrypt_secret(&reference.secret_id, reference.version)?,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SecretVersionState {
    Active,
    Revoked,
}

#[derive(Clone, Default)]
struct SecretCatalog {
    versions: HashMap<(SecretBackend, String, u64), SecretVersionState>,
    active: HashMap<(SecretBackend, String), u64>,
}

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

impl<R: SecretResolver> SecretManager<R> {
    pub fn new(resolver: R) -> Self {
        Self {
            resolver,
            catalog: RwLock::new(SecretCatalog::default()),
            durable: None,
        }
    }

    pub fn new_durable(resolver: R, catalog_path: impl AsRef<Path>) -> KernelResult<Self> {
        let mut durable = DurableSecretCatalog::open(catalog_path.as_ref())?;
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
}
