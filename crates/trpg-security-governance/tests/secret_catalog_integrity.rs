use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value;
use sha2::{Digest, Sha256};
use trpg_security_governance::secret::{
    migrate_previous_secret_catalog, MountedFileSecretResolver, SecretManager, SecretReference,
};

static NEXT_DIR: AtomicU64 = AtomicU64::new(1);
const CHILD_ROOT: &str = "TRPG_SECRET_CATALOG_CHILD_ROOT";
const CHILD_PATH: &str = "TRPG_SECRET_CATALOG_CHILD_PATH";
const CHILD_ID: &str = "TRPG_SECRET_CATALOG_CHILD_ID";

fn test_root() -> PathBuf {
    let suffix = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "secret-catalog-integrity-{}-{suffix}",
        std::process::id()
    ));
    fs::create_dir_all(&root).unwrap();
    #[cfg(unix)]
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    root
}

fn companion(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}

fn open_manager(root: &Path, path: &Path) -> SecretManager<MountedFileSecretResolver> {
    SecretManager::new_durable(MountedFileSecretResolver::new(root).unwrap(), path).unwrap()
}

fn populated_catalog() -> (PathBuf, PathBuf, SecretReference) {
    let root = test_root();
    let path = root.join("secret-catalog.jsonl");
    let first = SecretReference::mounted("provider", 1).unwrap();
    let revoked = SecretReference::mounted("provider", 2).unwrap();
    let manager = open_manager(&root, &path);
    manager.register(&first).unwrap();
    manager.rotate(&first, &revoked).unwrap();
    manager.revoke(&revoked).unwrap();
    drop(manager);
    (root, path, revoked)
}

fn catalog_lines(path: &Path) -> Vec<String> {
    fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

fn write_lines(path: &Path, lines: &[String]) {
    fs::write(path, format!("{}\n", lines.join("\n"))).unwrap();
}

fn assert_tamper_rejected(mutate: impl FnOnce(&Path, &mut Vec<String>)) {
    let (root, path, _) = populated_catalog();
    let mut lines = catalog_lines(&path);
    mutate(&path, &mut lines);
    let rejected =
        SecretManager::new_durable(MountedFileSecretResolver::new(&root).unwrap(), &path).is_err();
    fs::remove_dir_all(root).unwrap();
    assert!(rejected, "tampered secret catalog must fail closed");
}

#[test]
fn secret_catalog_rejects_revocation_tail_truncation() {
    assert_tamper_rejected(|path, lines| {
        lines.truncate(2);
        write_lines(path, lines);
    });
}

#[test]
fn secret_catalog_rejects_reorder_copy_deletion_mutation_and_partial_line() {
    assert_tamper_rejected(|path, lines| {
        lines.swap(0, 1);
        write_lines(path, lines);
    });
    assert_tamper_rejected(|path, lines| {
        lines.insert(1, lines[0].clone());
        write_lines(path, lines);
    });
    assert_tamper_rejected(|path, lines| {
        lines.remove(1);
        write_lines(path, lines);
    });
    assert_tamper_rejected(|path, lines| {
        lines[0] = lines[0].replacen("provider", "attacker", 1);
        write_lines(path, lines);
    });
    assert_tamper_rejected(|path, _| {
        OpenOptions::new()
            .append(true)
            .open(path)
            .unwrap()
            .write_all(br#"{"schema_version":1"#)
            .unwrap();
    });
}

#[test]
fn secret_catalog_rejects_old_snapshot_and_crash_boundary_mismatches() {
    let root = test_root();
    let path = root.join("secret-catalog.jsonl");
    let first = SecretReference::mounted("provider", 1).unwrap();
    let second = SecretReference::mounted("provider", 2).unwrap();
    let manager = open_manager(&root, &path);
    manager.register(&first).unwrap();
    let old_log = fs::read(&path).unwrap();
    manager.rotate(&first, &second).unwrap();
    drop(manager);
    fs::write(&path, old_log).unwrap();
    let old_log_rejected =
        SecretManager::new_durable(MountedFileSecretResolver::new(&root).unwrap(), &path).is_err();
    fs::remove_dir_all(&root).unwrap();
    assert!(old_log_rejected, "an old log must not pass a newer anchor");

    let root = test_root();
    let path = root.join("secret-catalog.jsonl");
    let manager = open_manager(&root, &path);
    manager.register(&first).unwrap();
    let anchor_path = companion(&path, ".head");
    let old_anchor = fs::read(&anchor_path).unwrap();
    manager.rotate(&first, &second).unwrap();
    drop(manager);
    fs::write(&anchor_path, &old_anchor).unwrap();
    let stale_anchor_rejected =
        SecretManager::new_durable(MountedFileSecretResolver::new(&root).unwrap(), &path).is_err();
    fs::remove_dir_all(root).unwrap();
    assert!(
        stale_anchor_rejected,
        "a synced log without its new anchor must fail closed"
    );
    assert!(!old_anchor.is_empty());
}

#[test]
fn secret_catalog_ignores_uncommitted_anchor_temporary_file() {
    let (root, path, _) = populated_catalog();
    fs::write(companion(&path, ".head.tmp-crash"), b"uncommitted").unwrap();
    let reopened =
        SecretManager::new_durable(MountedFileSecretResolver::new(&root).unwrap(), &path).is_ok();
    fs::remove_dir_all(root).unwrap();
    assert!(
        reopened,
        "a pre-rename temporary file is not committed state"
    );
}

#[test]
fn secret_catalog_previous_format_migration_is_explicit_auditable_and_one_time() {
    let (root, path, revoked) = populated_catalog();
    let previous: Vec<_> = catalog_lines(&path)
        .iter()
        .map(|line| {
            let record: Value = serde_json::from_str(line).unwrap();
            serde_json::to_string(&record["mutation"]).unwrap()
        })
        .collect();
    let encoded = format!("{}\n", previous.join("\n"));
    fs::remove_file(companion(&path, ".head")).unwrap();
    fs::write(&path, encoded.as_bytes()).unwrap();
    let digest = format!("sha256:{:x}", Sha256::digest(encoded.as_bytes()));

    assert!(
        SecretManager::new_durable(MountedFileSecretResolver::new(&root).unwrap(), &path).is_err()
    );
    assert!(migrate_previous_secret_catalog(
        &path,
        previous.len() as u64,
        &format!("sha256:{}", "0".repeat(64))
    )
    .is_err());
    migrate_previous_secret_catalog(&path, previous.len() as u64, &digest).unwrap();
    assert!(migrate_previous_secret_catalog(&path, previous.len() as u64, &digest).is_err());

    let records: Vec<Value> = catalog_lines(&path)
        .iter()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert!(records
        .iter()
        .all(|record| record["source"] == "previous_format_migration"));
    let manager = open_manager(&root, &path);
    assert!(manager.resolve(&revoked).is_err());
    drop(manager);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn secret_catalog_child_process() {
    let Some(root) = std::env::var_os(CHILD_ROOT) else {
        return;
    };
    let path = std::env::var_os(CHILD_PATH).unwrap();
    let secret_id = std::env::var(CHILD_ID).unwrap();
    open_manager(Path::new(&root), Path::new(&path))
        .register(&SecretReference::mounted(secret_id, 1).unwrap())
        .unwrap();
}

#[test]
fn secret_catalog_serializes_multiprocess_writers() {
    let root = test_root();
    let path = root.join("secret-catalog.jsonl");
    drop(open_manager(&root, &path));
    let executable = std::env::current_exe().unwrap();
    let mut children = Vec::new();
    for index in 0..8 {
        children.push(
            Command::new(&executable)
                .args(["--exact", "secret_catalog_child_process", "--nocapture"])
                .env(CHILD_ROOT, &root)
                .env(CHILD_PATH, &path)
                .env(CHILD_ID, format!("concurrent-provider-{index}"))
                .spawn()
                .unwrap(),
        );
    }
    for child in children {
        assert!(child.wait_with_output().unwrap().status.success());
    }
    drop(open_manager(&root, &path));
    let records: Vec<Value> = catalog_lines(&path)
        .iter()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(records.len(), 8);
    assert!(records
        .iter()
        .enumerate()
        .all(|(index, record)| record["sequence"] == index as u64 + 1));
    assert_eq!(
        records
            .iter()
            .map(|record| record["record_hash"].as_str().unwrap())
            .collect::<HashSet<_>>()
            .len(),
        8
    );
    fs::remove_dir_all(root).unwrap();
}
