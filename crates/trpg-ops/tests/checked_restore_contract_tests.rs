use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use trpg_ops::backup_restore_runbook::{PostgresBackupExecutor, PostgresBackupRestoreError};

struct TestRoot(PathBuf);

impl TestRoot {
    fn new() -> Self {
        static SEQUENCE: AtomicU64 = AtomicU64::new(1);
        let path = std::env::temp_dir().join(format!(
            "trpg-checked-restore-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).expect("create test root");
        Self(path)
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn checked_restore_rejects_hash_and_schema_before_creating_safety_point() {
    let root = TestRoot::new();
    let safety = root.0.join("safety");
    let executor = test_executor(&root.0, &safety);
    let artifact = executor
        .create_backup(
            "source_service",
            root.0.join("backups"),
            "backup_one",
            "schema_v1",
        )
        .expect("create test backup");

    assert_eq!(
        executor
            .verify_backup(&artifact.manifest_path, "schema_v2")
            .expect_err("schema mismatch must fail"),
        PostgresBackupRestoreError::ManifestInvalid
    );
    assert!(!safety.exists());

    fs::write(&artifact.archive_path, b"tampered archive").expect("tamper archive");
    assert_eq!(
        executor
            .restore_backup_checked(
                "restore_target",
                &artifact.manifest_path,
                "schema_v1",
                &safety,
                "before_restore"
            )
            .expect_err("hash mismatch must fail"),
        PostgresBackupRestoreError::ArchiveHashMismatch
    );
    assert!(!safety.exists());
}

#[test]
fn checked_restore_publishes_safety_point_before_restore() {
    let root = TestRoot::new();
    let safety = root.0.join("safety");
    let executor = test_executor(&root.0, &safety);
    let artifact = executor
        .create_backup(
            "source_service",
            root.0.join("backups"),
            "backup_two",
            "schema_v1",
        )
        .expect("create test backup");
    let restored = executor
        .restore_backup_checked(
            "restore_target",
            &artifact.manifest_path,
            "schema_v1",
            &safety,
            "before_restore",
        )
        .expect("checked restore");
    assert_eq!(restored.restored_manifest.backup_id, "backup_two");
    assert!(restored.safety_point.archive_path.is_file());
    assert!(restored.safety_point.manifest_path.is_file());
}

fn test_executor(root: &Path, safety: &Path) -> PostgresBackupExecutor {
    let dump = root.join("pg_dump");
    let restore = root.join("pg_restore");
    let service = root.join("pg_service.conf");
    write_executable(
        &dump,
        r#"#!/bin/sh
set -eu
if [ "${1:-}" = "--version" ]; then
  printf '%s\n' 'pg_dump (PostgreSQL) 17.0'
  exit 0
fi
output=''
while [ "$#" -gt 0 ]; do
  if [ "$1" = "--file" ]; then
    shift
    output="$1"
  fi
  shift
done
[ -n "$output" ]
printf '%s\n' 'deterministic custom dump' > "$output"
"#,
    );
    let safety_manifest = safety.join("before_restore.manifest.json");
    write_executable(
        &restore,
        &format!(
            r#"#!/bin/sh
set -eu
if [ "${{1:-}}" = "--version" ]; then
  printf '%s\n' 'pg_restore (PostgreSQL) 17.0'
  exit 0
fi
if [ "${{1:-}}" = "--list" ]; then
  printf '%s\n' '2; 3079 16639 EXTENSION - vector '
  printf '%s\n' '3; 0 0 COMMENT - EXTENSION vector '
  exit 0
fi
[ -f '{}' ]
"#,
            safety_manifest.display()
        ),
    );
    fs::write(&service, b"[source_service]\n[restore_target]\n").expect("write service file");
    PostgresBackupExecutor::new(dump, restore, service, None).expect("backup executor")
}

fn write_executable(path: &Path, body: &str) {
    use std::os::unix::fs::PermissionsExt as _;
    fs::write(path, body).expect("write executable");
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).expect("chmod executable");
}
