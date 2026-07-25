use std::env;
use std::fs;

use postgres::{Client, NoTls};
use trpg_ops::backup_restore_runbook::{PostgresBackupExecutor, PostgresBackupRestoreError};

#[test]
fn custom_format_backup_restores_to_an_independent_database_and_detects_tampering() {
    let pg_dump =
        env::var("P02_PG_DUMP").expect("P02_PG_DUMP is required for the real backup gate");
    let pg_restore =
        env::var("P02_PG_RESTORE").expect("P02_PG_RESTORE is required for the real restore gate");
    let service_file = env::var("P02_LIBPQ_SERVICE_FILE")
        .expect("P02_LIBPQ_SERVICE_FILE is required for credential-safe backup connections");
    let source_service = env::var("P02_BACKUP_SOURCE_SERVICE")
        .expect("P02_BACKUP_SOURCE_SERVICE is required for the real backup gate");
    let target_service = env::var("P02_BACKUP_TARGET_SERVICE")
        .expect("P02_BACKUP_TARGET_SERVICE is required for the independent restore gate");
    let source_url = env::var("P02_BACKUP_SOURCE_URL")
        .expect("P02_BACKUP_SOURCE_URL is required to verify the source database");
    let target_url = env::var("P02_BACKUP_TARGET_URL")
        .expect("P02_BACKUP_TARGET_URL is required to verify the restored database");
    let output_dir = env::var("P02_BACKUP_DIR")
        .expect("P02_BACKUP_DIR is required for the real backup artifact");

    let executor = PostgresBackupExecutor::new(pg_dump, pg_restore, service_file, None).unwrap();
    let backup_id = format!("p02_backup_{}", std::process::id());
    let artifact = executor
        .create_backup(
            &source_service,
            &output_dir,
            &backup_id,
            "p02_schema_current",
        )
        .unwrap();
    assert!(artifact.manifest.sha256.starts_with("sha256:"));
    assert!(artifact.manifest.byte_length > 0);
    executor
        .restore_backup(&target_service, &artifact.manifest_path)
        .unwrap();
    executor
        .restore_backup(&target_service, &artifact.manifest_path)
        .expect("the independent recovery target must support a repeat restore");

    let mut source = Client::connect(&source_url, NoTls).unwrap();
    let mut target = Client::connect(&target_url, NoTls).unwrap();
    for table in [
        "event_store",
        "event_outbox",
        "canonical_audit_log",
        "formal_commits",
    ] {
        let query = format!("SELECT count(*)::bigint FROM {table}");
        let source_count: i64 = source.query_one(&query, &[]).unwrap().get(0);
        let target_count: i64 = target.query_one(&query, &[]).unwrap().get(0);
        assert_eq!(target_count, source_count, "restored {table} count");
    }

    let tampered_manifest_path = artifact
        .manifest_path
        .with_file_name(format!("{backup_id}.tampered.manifest.json"));
    fs::copy(&artifact.manifest_path, &tampered_manifest_path).unwrap();
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&tampered_manifest_path).unwrap()).unwrap();
    manifest["sha256"] = serde_json::Value::String(format!("sha256:{}", "0".repeat(64)));
    fs::write(
        &tampered_manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    assert_eq!(
        executor
            .restore_backup("unused_target", &tampered_manifest_path)
            .unwrap_err(),
        PostgresBackupRestoreError::ArchiveHashMismatch
    );
}
