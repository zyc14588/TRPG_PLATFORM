use std::collections::HashMap;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, RwLock};

use sha2::{Digest, Sha256};
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
