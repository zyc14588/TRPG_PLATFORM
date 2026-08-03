#[test]
fn s09_release_process_smoke_uses_the_production_secret_boundary() {
    for required in [
        "TRPG_SECRET_MOUNT",
        "TRPG_SECRET_CATALOG_PATH",
        "TRPG_DATABASE_URL_SECRET_ID",
        "TRPG_WITNESS_DATABASE_URL_SECRET_ID",
        "TRPG_CANONICAL_HMAC_KEY_SECRET_ID",
        "TRPG_PAYLOAD_ENCRYPTION_KEY_SECRET_ID",
        "TRPG_IDENTITY_SIGNING_KEY_SECRET_ID",
        "TRPG_AUDIT_HMAC_KEY_SECRET_ID",
        "TRPG_ADMIN_BOOTSTRAP_TOKEN_SECRET_ID",
        "TRPG_ADMIN_STATE_PATH",
        "TRPG_ADMIN_AUDIT_LOG_PATH",
        "TRPG_ADMIN_PSQL_PATH",
        "TRPG_ADMIN_PG_DUMP_PATH",
        "TRPG_ADMIN_PG_RESTORE_PATH",
        "TRPG_REDIS_CACHE_KEY_ID",
        "TRPG_MODEL_PROVIDER_TIMEOUT_MS",
        "TRPG_OBJECT_STORAGE_ACCESS_KEY_SECRET_ID",
        "TRPG_OBJECT_STORAGE_SECRET_KEY_SECRET_ID",
    ] {
        assert!(
            PROCESS_SMOKE.contains(required),
            "release process smoke omits {required}"
        );
    }
    for forbidden in [
        "TRPG_CANONICAL_HMAC_KEY_HEX",
        "TRPG_PAYLOAD_ENCRYPTION_KEY_HEX",
        "TRPG_IDENTITY_SIGNING_KEY_HEX",
        "TRPG_AUDIT_HMAC_KEY_HEX",
    ] {
        assert!(
            !PROCESS_SMOKE.contains(forbidden),
            "release process smoke still injects {forbidden}"
        );
    }
    assert!(PROCESS_SMOKE.contains("install -d -m 0700"));
    assert!(PROCESS_SMOKE.contains("umask 077"));
    assert!(
        PROCESS_SMOKE.contains(
            "secret_catalog_path=\"$secret_catalog_directory/$service/catalog.jsonl\""
        ),
        "each release process must use the service-isolated catalog provided by its production state volume"
    );
    assert!(
        PROCESS_SMOKE.contains("install -d -m 0700 \"$secret_catalog_directory/$service\""),
        "each service catalog parent must retain private directory permissions"
    );
    assert_eq!(
        shell_array_entries(PROCESS_SMOKE, "component_checks"),
        [
            "api_runtime",
            "realtime_runtime",
            "agent_worker_runtime",
            "admin_runtime",
            "migration_runtime",
        ],
        "release readiness checks must remain aligned with the five service processes"
    );
    let admin_environment = PROCESS_SMOKE
        .rsplit_once("if [[ \"$service\" == admin-server ]]; then")
        .map(|(_, block)| block)
        .and_then(|block| block.split_once("\n  fi").map(|(body, _)| body))
        .expect("admin-server environment block exists");
    for required in [
        "TRPG_REDIS_URL_SECRET_ID=redis_url",
        "TRPG_IDENTITY_SIGNING_KEY_SECRET_ID=identity_signing_key",
        "TRPG_ADMIN_BOOTSTRAP_TOKEN_SECRET_ID=admin_bootstrap_token",
        "TRPG_AUDIT_HMAC_KEY_SECRET_ID=audit_hmac_key",
        "TRPG_ADMIN_BACKUP_SOURCE_SERVICE=trpg_backup_source",
        "TRPG_ADMIN_RESTORE_TARGET_SERVICE=trpg_backup_target",
    ] {
        assert!(
            admin_environment.contains(required),
            "admin-server production smoke omits {required}"
        );
    }
    for worker_secret in [
        "database_secret_id=\"worker_database_url\"",
        "witness_database_secret_id=\"worker_witness_database_url\"",
        "TRPG_CANONICAL_DATABASE_URL_SECRET_ID=canonical_database_url",
    ] {
        assert!(
            PROCESS_SMOKE.contains(worker_secret),
            "agent-worker must use its least-privilege {worker_secret} binding"
        );
    }
    for role_url in [
        "P02_WORKER_SERVICE_DATABASE_URL=postgresql://trpg_worker_login:",
        "P02_CANONICAL_SERVICE_DATABASE_URL=postgresql://trpg_canonical_login:",
        "P02_WITNESS_APPEND_DATABASE_URL=postgresql://trpg_witness_append_login:",
    ] {
        assert!(
            INTEGRATION_SERVICES.contains(role_url),
            "integration services must provision the least-privilege {role_url} binding"
        );
    }
    let agent_worker_environment = PROCESS_SMOKE
        .rsplit_once("if [[ \"$service\" == agent-worker ]]; then")
        .map(|(_, block)| block)
        .and_then(|block| block.split_once("\n  fi").map(|(body, _)| body))
        .expect("agent-worker environment block exists");
    assert!(
        !agent_worker_environment.contains("TRPG_CANONICAL_DATABASE_URL_SECRET_ID=database_url"),
        "agent-worker must not reuse the owner/API database secret for canonical custody"
    );
}
