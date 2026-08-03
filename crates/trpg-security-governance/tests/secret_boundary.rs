use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::sync::atomic::{AtomicU64, Ordering};

use trpg_security_governance::secret::{
    KmsClient, KmsSecretResolver, MountedFileSecretResolver, SecretManager, SecretReference,
};
use trpg_shared_kernel::TrpgError;

#[path = "common/ledger_checkpoint_store.rs"]
mod ledger_checkpoint_store;
use ledger_checkpoint_store::TestFileCheckpointStore;

static NEXT_DIR: AtomicU64 = AtomicU64::new(1);
const INTEGRITY_KEY: [u8; 32] = [0x39; 32];

#[test]
fn mounted_secret_is_redacted_rotatable_and_revocable() {
    let suffix = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "p05-secret-boundary-{}-{suffix}",
        std::process::id()
    ));
    fs::create_dir_all(&root).unwrap();
    #[cfg(unix)]
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let first_secret_path = root.join("cloud_provider.v1");
    let second_secret_path = root.join("cloud_provider.v2");
    let first_material = b"first-provider-material";
    let second_material = b"second-provider-material";
    fs::write(&first_secret_path, first_material).unwrap();
    fs::write(&second_secret_path, second_material).unwrap();
    #[cfg(unix)]
    {
        fs::set_permissions(&first_secret_path, fs::Permissions::from_mode(0o600)).unwrap();
        fs::set_permissions(&second_secret_path, fs::Permissions::from_mode(0o600)).unwrap();
    }

    let first = SecretReference::mounted("cloud_provider", 1).unwrap();
    let second = SecretReference::mounted("cloud_provider", 2).unwrap();
    let debug = format!("{first:?}");
    assert!(!debug.contains("cloud_provider"));
    assert!(!debug.contains(std::str::from_utf8(first_material).unwrap()));

    let catalog_path = root.join("secret-revocation-ledger.jsonl");
    let manager = SecretManager::new_durable_with_checkpoint(
        MountedFileSecretResolver::new(&root).unwrap(),
        &catalog_path,
        "test-secret-catalog-key",
        &INTEGRITY_KEY,
        TestFileCheckpointStore::shared(root.join("secret-revocation-ledger.witness")),
    )
    .unwrap();
    manager.register(&first).unwrap();
    let lease = manager.resolve(&first).unwrap();
    lease.expose_to(|material| assert_eq!(material, first_material));

    manager.rotate(&first, &second).unwrap();
    assert!(matches!(
        manager.resolve(&first),
        Err(TrpgError::AuthorizationDenied)
    ));
    manager
        .resolve(&second)
        .unwrap()
        .expose_to(|material| assert_eq!(material, second_material));

    manager.revoke(&second).unwrap();
    assert!(matches!(
        manager.resolve(&second),
        Err(TrpgError::AuthorizationDenied)
    ));
    assert!(manager.register(&second).is_err());
    assert!(manager
        .register(&SecretReference::mounted("cloud_provider", 3).unwrap())
        .is_err());

    drop(manager);
    let restarted = SecretManager::new_durable_with_checkpoint(
        MountedFileSecretResolver::new(&root).unwrap(),
        &catalog_path,
        "test-secret-catalog-key",
        &INTEGRITY_KEY,
        TestFileCheckpointStore::shared(root.join("secret-revocation-ledger.witness")),
    )
    .unwrap();
    assert!(matches!(
        restarted.resolve(&second),
        Err(TrpgError::AuthorizationDenied)
    ));
    assert!(
        restarted.register(&second).is_err(),
        "a process restart must not resurrect a revoked mounted secret"
    );

    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn mounted_secret_resolution_is_anchored_to_directory_fd_and_never_follows_symlinks() {
    use std::os::unix::fs::symlink;

    let suffix = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "p05-secret-atomic-root-{}-{suffix}",
        std::process::id()
    ));
    let replacement = std::env::temp_dir().join(format!(
        "p05-secret-atomic-replacement-{}-{suffix}",
        std::process::id()
    ));
    let moved_root = root.with_extension("anchored");
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(&replacement).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    fs::set_permissions(&replacement, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(root.join("provider.v1"), b"anchored-secret").unwrap();
    fs::write(replacement.join("provider.v1"), b"attacker-secret").unwrap();
    fs::set_permissions(root.join("provider.v1"), fs::Permissions::from_mode(0o600)).unwrap();
    fs::set_permissions(
        replacement.join("provider.v1"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();

    let reference = SecretReference::mounted("provider", 1).unwrap();
    let manager = SecretManager::new(MountedFileSecretResolver::new(&root).unwrap());
    manager.register(&reference).unwrap();
    fs::rename(&root, &moved_root).unwrap();
    symlink(&replacement, &root).unwrap();
    manager
        .resolve(&reference)
        .unwrap()
        .expose_to(|bytes| assert_eq!(bytes, b"anchored-secret"));

    fs::remove_file(&root).unwrap();
    fs::remove_dir_all(&moved_root).unwrap();
    fs::remove_dir_all(&replacement).unwrap();
}

#[cfg(unix)]
#[test]
fn mounted_secret_rejects_a_symlink_entry() {
    use std::os::unix::fs::symlink;

    let suffix = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "p05-secret-symlink-root-{}-{suffix}",
        std::process::id()
    ));
    let outside = root.with_extension("outside");
    fs::create_dir_all(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(&outside, b"outside-secret").unwrap();
    fs::set_permissions(&outside, fs::Permissions::from_mode(0o600)).unwrap();
    symlink(&outside, root.join("provider.v1")).unwrap();
    let reference = SecretReference::mounted("provider", 1).unwrap();
    let manager = SecretManager::new(MountedFileSecretResolver::new(&root).unwrap());
    manager.register(&reference).unwrap();

    assert!(manager.resolve(&reference).is_err());

    fs::remove_dir_all(&root).unwrap();
    fs::remove_file(&outside).unwrap();
}

struct VersionedKms;

impl KmsClient for VersionedKms {
    fn decrypt_secret(&self, secret_id: &str, version: u64) -> Result<Vec<u8>, TrpgError> {
        if secret_id != "cloud_provider" {
            return Err(TrpgError::AuthorizationDenied);
        }
        Ok(format!("kms-material-v{version}").into_bytes())
    }
}

#[test]
fn kms_secret_rotation_and_revocation_never_expose_material_in_debug() {
    let first = SecretReference::kms("cloud_provider", 1).unwrap();
    let second = SecretReference::kms("cloud_provider", 2).unwrap();
    let manager = SecretManager::new(KmsSecretResolver::new(VersionedKms));
    manager.register(&first).unwrap();
    manager
        .resolve(&first)
        .unwrap()
        .expose_to(|bytes| assert_eq!(bytes, b"kms-material-v1"));
    manager.rotate(&first, &second).unwrap();
    assert!(manager.resolve(&first).is_err());
    manager
        .resolve(&second)
        .unwrap()
        .expose_to(|bytes| assert_eq!(bytes, b"kms-material-v2"));
    assert!(!format!("{second:?}").contains("kms-material"));
    manager.revoke(&second).unwrap();
    assert!(manager.resolve(&second).is_err());
}

#[test]
fn production_accepts_only_kms_or_secret_mount_references() {
    assert!(SecretReference::kms("cloud_provider", 1)
        .unwrap()
        .production_eligible());
    assert!(SecretReference::mounted("local_provider", 1)
        .unwrap()
        .production_eligible());
    assert!(!SecretReference::development("dev_provider", 1)
        .unwrap()
        .production_eligible());
    assert_eq!(
        SecretReference::mounted("../escape", 1).unwrap_err(),
        TrpgError::InvalidConfiguration("invalid_secret_reference")
    );
}
