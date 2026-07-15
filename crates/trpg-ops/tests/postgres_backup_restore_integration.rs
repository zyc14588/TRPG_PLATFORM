use std::env;
use std::fs;

use postgres::{Client, NoTls};
use trpg_ops::backup_restore_runbook::{PostgresBackupExecutor, PostgresBackupRestoreError};

#[test]
fn custom_format_backup_restores_to_an_independent_database_and_detects_tampering() {
    let (
        Ok(pg_dump),
        Ok(pg_restore),
        Ok(service_file),
        Ok(source_service),
        Ok(target_service),
        Ok(source_url),
        Ok(target_url),
        Ok(output_dir),
    ) = (
        env::var("P02_PG_DUMP"),
        env::var("P02_PG_RESTORE"),
        env::var("P02_LIBPQ_SERVICE_FILE"),
        env::var("P02_BACKUP_SOURCE_SERVICE"),
        env::var("P02_BACKUP_TARGET_SERVICE"),
        env::var("P02_BACKUP_SOURCE_URL"),
        env::var("P02_BACKUP_TARGET_URL"),
        env::var("P02_BACKUP_DIR"),
    )
    else {
        eprintln!("skipped: set P02 pg_dump/pg_restore and independent source/target databases");
        return;
    };

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
