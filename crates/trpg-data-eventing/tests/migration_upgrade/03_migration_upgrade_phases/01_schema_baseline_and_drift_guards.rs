{
    let mut pool = test_pool().await;
    let current = persistence_migrations::migrator();
    let frozen = current
        .iter()
        .find(|migration| {
            migration.migration_type.is_up_migration()
                && migration.version
                    == sqlx_migrations_contract::FROZEN_EVENT_STORE_MIGRATION_VERSION
        })
        .expect("frozen migration compiled by sqlx::migrate!");
    assert_eq!(
        checksum_hex(&frozen.checksum),
        sqlx_migrations_contract::FROZEN_EVENT_STORE_MIGRATION_SHA384
    );
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/b24");
    let b24 = Migrator::new(fixture_path.as_path())
        .await
        .expect("resolve historical b-24 fixture");
    assert_eq!(b24.iter().count(), 1);
    assert_eq!(
        checksum_hex(&b24.iter().next().unwrap().checksum),
        sqlx_migrations_contract::FROZEN_EVENT_STORE_MIGRATION_SHA384
    );
    let b25_fixture_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/b25");
    let b25 = Migrator::new(b25_fixture_path.as_path())
        .await
        .expect("resolve observed checksum-drift fixture");
    assert_eq!(
        checksum_hex(&b25.iter().next().unwrap().checksum),
        "7cd30a91cb521ba1288303287d8cac9674a65d81630c741480e708b58e799598ee6fa63509c49c064edd5ea200f376e7"
    );

    // Empty database -> HEAD, followed by a true SQLx no-op repeat.
    reset_database(&pool).await;
    current.run(&pool).await.expect("empty database migration");
    assert_schema(&pool).await;
    let ledger_before: i64 = sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .unwrap();
    current
        .run(&pool)
        .await
        .expect("repeat migration is a no-op");
    let ledger_after: i64 = sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(ledger_before, ledger_after);

    // A forward upgrade can contain append-only cloud-route evidence created
    // after the first P05 hardening migration. The route-binding migration
    // must lock the tables, perform its owner-authorized backfill, and restore
    // the append-only guards before commit.
    reset_database(&pool).await;
    migrator_through(current, 20260721000200)
        .run(&pool)
        .await
        .expect("apply through append-only cloud evidence schema");
    sqlx::raw_sql(
        r#"
        INSERT INTO cloud_egress_route_snapshots (
            snapshot_id, subject_id, consent_id, source_provider,
            target_provider, purpose, policy_version, notice_reference,
            context_manifest_hash, allowed_fact_ids, decision, denial_code,
            created_at_unix_ms
        ) VALUES (
            'populated_upgrade_route', 'subject_upgrade', NULL,
            'local-provider', 'cloud-provider', 'model_assistance',
            'privacy-v1', NULL,
            'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
            '[]'::jsonb, 'deny', 'CONSENT_REQUIRED', 1000
        );
        INSERT INTO cloud_egress_audit (
            audit_id, snapshot_id, subject_id, decision, denial_code,
            context_manifest_hash, created_at_unix_ms
        ) VALUES (
            'populated_upgrade_audit', 'populated_upgrade_route',
            'subject_upgrade', 'deny', 'CONSENT_REQUIRED',
            'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
            1000
        );
        "#,
    )
    .execute(&pool)
    .await
    .expect("seed retained append-only cloud evidence");
    current
        .run(&pool)
        .await
        .expect("populated append-only cloud evidence upgrades to HEAD");
    let upgraded_route: (String, String, String, String) = sqlx::query_as(
        "SELECT source_endpoint, target_endpoint, fallback_policy, privacy_boundary \
           FROM cloud_egress_route_snapshots \
          WHERE snapshot_id = 'populated_upgrade_route'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        upgraded_route,
        (
            "http://127.0.0.1/historical-unavailable".to_owned(),
            "https://historical.invalid".to_owned(),
            "historical_unavailable".to_owned(),
            "historical_unavailable".to_owned(),
        )
    );
    assert!(sqlx::query(
        "UPDATE cloud_egress_route_snapshots \
            SET target_endpoint = 'https://mutated.invalid' \
          WHERE snapshot_id = 'populated_upgrade_route'",
    )
    .execute(&pool)
    .await
    .is_err());

    // A legacy schema may have lost the original anonymous key-material
    // constraint. The lease migration must not silently destroy material to
    // make such drift pass: it fails before recording success and leaves the
    // incident row untouched for authorized remediation.
    reset_database(&pool).await;
    migrator_through(current, 20260724000300)
        .run(&pool)
        .await
        .expect("apply through the migration before deletion leases");
    pool.execute(
        "ALTER TABLE privacy_subject_keys \
             DROP CONSTRAINT privacy_subject_keys_check",
    )
    .await
    .expect("simulate a legacy schema missing the anonymous material check");
    sqlx::query(
        r#"
        INSERT INTO privacy_subject_keys (
            subject_id, key_reference, wrapped_key, destroyed_at
        ) VALUES (
            'legacy_invalid_destroyed_subject', 'legacy_key_reference',
            decode('aabbccdd', 'hex'), statement_timestamp()
        )
        "#,
    )
    .execute(&pool)
    .await
    .expect("seed a claimed-destroyed key that still contains material");
    let invalid_key_error = current
        .run(&pool)
        .await
        .expect_err("migration must reject retained destroyed key material");
    assert!(
        invalid_key_error
            .to_string()
            .contains("privacy subject key material remains after a claimed destruction"),
        "unexpected key-material preflight error: {invalid_key_error}"
    );
    let retained_material: Option<Vec<u8>> = sqlx::query_scalar(
        "SELECT wrapped_key FROM privacy_subject_keys \
          WHERE subject_id = 'legacy_invalid_destroyed_subject'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        retained_material,
        Some(vec![0xaa, 0xbb, 0xcc, 0xdd]),
        "failed migration must not silently destroy legacy key material"
    );
    let lease_migration_success: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM _sqlx_migrations \
          WHERE version = 20260725000100 AND success",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        lease_migration_success, 0,
        "failed key-material preflight must not record migration success"
    );
    pool.close().await;
    pool = test_pool().await;

    // Restore a clean HEAD schema for the independent drift probes below.
    reset_database(&pool).await;
    current
        .run(&pool)
        .await
        .expect("restore clean HEAD after populated cloud upgrade probe");

    // Exact trigger fingerprints must include the WHEN predicate. The
    // trigger type and target function OID are unchanged by this bypass.
    sqlx::raw_sql(
        r#"
        DROP TRIGGER event_store_append_only ON event_store;
        CREATE TRIGGER event_store_append_only
        BEFORE UPDATE OR DELETE ON event_store
        FOR EACH ROW WHEN (false)
        EXECUTE FUNCTION reject_canonical_append_mutation();
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();
    assert_schema_rejects(
        &pool,
        "trigger relation/enabled/definition signature drifted",
    )
    .await;

    reset_database(&pool).await;
    current
        .run(&pool)
        .await
        .expect("restore canonical schema after trigger-predicate drift probe");
    sqlx::raw_sql(
        r#"
        ALTER FUNCTION enforce_event_outbox_binding() SECURITY DEFINER;
        ALTER FUNCTION enforce_event_outbox_binding()
            SET search_path TO pg_catalog, public;
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();
    assert_schema_rejects(
        &pool,
        "trigger function definition/execution signature drifted",
    )
    .await;

    // A database that already recorded the observed b-25 rewrite is not
    // silently blessed or ledger-edited. SQLx must stop at the immutable
    // version with an explicit checksum mismatch.
    reset_database(&pool).await;
    b25.run(&pool)
        .await
        .expect("apply observed rewritten fixture");
    assert!(matches!(
        current.run(&pool).await,
        Err(sqlx::migrate::MigrateError::VersionMismatch(version))
            if version == sqlx_migrations_contract::FROZEN_EVENT_STORE_MIGRATION_VERSION
    ));
    // A migration checksum mismatch can return before SQLx releases its
    // session-level advisory lock. This dedicated test must not reuse that
    // pooled session for the next independent upgrade scenario.
    pool.close().await;
    pool = test_pool().await;

    // The old IF NOT EXISTS path must no longer turn a drifted historical
    // schema into success.  Start from a valid b-24 ledger, alter a critical
    // type before P03, and prove the hardening migration rejects it without a
    // successful ledger row.
    reset_database(&pool).await;
    b24.run(&pool).await.expect("apply b-24 before drift probe");
    pool.execute(
        "ALTER TABLE event_store ALTER COLUMN payload_json TYPE JSONB USING payload_json::jsonb",
    )
    .await
    .expect("create deliberate pre-P03 type drift");
    assert!(current.run(&pool).await.is_err());
    assert_hardening_not_recorded(&pool).await;
    pool.close().await;
    pool = test_pool().await;

    // A semantically disabled trigger retains the same name, event mask and
    // target function. The preflight must still reject its WHEN predicate
    // before applying any P03 DDL or recording a successful ledger row.
    reset_database(&pool).await;
    migrator_before_p03(current)
        .run(&pool)
        .await
        .expect("apply the canonical schema before trigger-predicate drift probe");
    sqlx::raw_sql(
        r#"
        DROP TRIGGER event_store_append_only ON event_store;
        CREATE TRIGGER event_store_append_only
        BEFORE UPDATE OR DELETE ON event_store
        FOR EACH ROW WHEN (false)
        EXECUTE FUNCTION reject_canonical_append_mutation();
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();
    let predicate_drift_error = current
        .run(&pool)
        .await
        .expect_err("trigger WHEN drift must fail before P03");
    assert!(predicate_drift_error
        .to_string()
        .contains("mutation trigger drift"));
    assert_hardening_not_recorded(&pool).await;
    pool.close().await;
    pool = test_pool().await;

    // CREATE OR REPLACE preserves the function OID. Preflight therefore must
    // verify function semantics, not merely that every trigger still points to
    // the same OID/name/signature.
    reset_database(&pool).await;
    migrator_before_p03(current)
        .run(&pool)
        .await
        .expect("apply the canonical schema before mutation-function drift probe");
    let mutation_function_oid_before: i64 = sqlx::query_scalar(
        "SELECT 'reject_canonical_append_mutation()'::regprocedure::oid::bigint",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::raw_sql(
        r#"
        CREATE OR REPLACE FUNCTION reject_canonical_append_mutation()
        RETURNS trigger LANGUAGE plpgsql AS $body$
        BEGIN
            RETURN NEW;
        END;
        $body$;
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();
    let mutation_function_oid_after: i64 = sqlx::query_scalar(
        "SELECT 'reject_canonical_append_mutation()'::regprocedure::oid::bigint",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(mutation_function_oid_before, mutation_function_oid_after);
    let mutation_function_error = current
        .run(&pool)
        .await
        .expect_err("same-OID mutation function drift must fail before P03");
    assert!(mutation_function_error
        .to_string()
        .contains("mutation function drift"));
    assert_hardening_not_recorded(&pool).await;
    pool.close().await;
    pool = test_pool().await;

    reset_database(&pool).await;
    include!("02_pre_p03_constraint_repairs.rs");
}
