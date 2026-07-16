use std::borrow::Cow;
use std::env;
use std::path::Path;
use std::str::FromStr;

use serde_json::{json, Value};
use sqlx::migrate::Migrator;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions, PgQueryResult};
use sqlx::{Executor, PgPool, Postgres, Transaction};
use trpg_data_eventing::event_store_sqlx_outbox_projection::load_canonical_replay_page;
use trpg_data_eventing::persistence::{
    EventOutboxRecord, EventPayloadUpcaster, EventStoreRecord, ProjectionCheckpointRecord,
    CURRENT_EVENT_SCHEMA_VERSION,
};
use trpg_data_eventing::{persistence_migrations, sqlx_migrations_contract};

const ZERO_REQUEST_HASH: &str =
    "sha256:0000000000000000000000000000000000000000000000000000000000000000";
const REQUEST_HASH_A: &str =
    "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const REQUEST_HASH_B: &str =
    "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

fn checksum_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

async fn test_pool() -> PgPool {
    assert_eq!(
        env::var("P03_ALLOW_DATABASE_RESET").as_deref(),
        Ok("1"),
        "set P03_ALLOW_DATABASE_RESET=1 to authorize this destructive migration test"
    );
    let url = env::var("P03_DATABASE_URL")
        .expect("P03_DATABASE_URL must name the dedicated p03_migration_upgrade database");
    let options = PgConnectOptions::from_str(&url).expect("valid PostgreSQL URL");
    let database = options.get_database().unwrap_or_default();
    assert_eq!(
        database, "p03_migration_upgrade",
        "migration_upgrade refuses to reset any database except p03_migration_upgrade"
    );
    PgPoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await
        .expect("connect to dedicated P03 PostgreSQL database")
}

async fn reset_database(pool: &PgPool) {
    sqlx::raw_sql(
        "DROP SCHEMA public CASCADE; CREATE SCHEMA public; GRANT ALL ON SCHEMA public TO public; CREATE EXTENSION IF NOT EXISTS vector;",
    )
    .execute(pool)
    .await
    .expect("reset dedicated P03 database");
}

async fn assert_hardening_not_recorded(pool: &PgPool) {
    let hardened: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM _sqlx_migrations WHERE version = 20260716000100 AND success",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(
        hardened, 0,
        "failed preflight must not write a success ledger row"
    );
}

fn migrator_before_p03(current: &Migrator) -> Migrator {
    Migrator {
        migrations: Cow::Owned(
            current
                .iter()
                .filter(|migration| migration.version < 20260716000100)
                .cloned()
                .collect(),
        ),
        ..Migrator::DEFAULT
    }
}

async fn insert_pre_p03_audit(pool: &PgPool, commit_id: &str, campaign_id: &str, tag: char) -> i64 {
    let digest = tag.to_string().repeat(64);
    let previous: Option<(i64, String)> = sqlx::query_as(
        "SELECT sequence, record_hash FROM canonical_audit_log ORDER BY sequence DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .unwrap();
    let (sequence, previous_hash) = previous.map_or_else(
        || (1, format!("hmac-sha256:{}", "0".repeat(64))),
        |(sequence, record_hash)| {
            (
                sequence
                    .checked_add(1)
                    .expect("audit fixture sequence fits"),
                record_hash,
            )
        },
    );
    sqlx::query_scalar(
        r#"
        INSERT INTO canonical_audit_log (
            sequence, commit_id, campaign_id, actor_id, actor_origin,
            authentication_reference, resource_type, resource_id, action,
            requested_role, visibility_label, visibility_subject,
            provenance_kind, provenance_reference, provenance_recorded_by,
            decision, openfga_decision_id, openfga_policy_revision,
            opa_decision_id, opa_policy_revision, trace_id, event_batch_hash,
            witness_prepare_sequence, witness_prepare_hash, integrity_key_id,
            previous_hash, record_hash
        ) VALUES (
            $1, $2, $3, 'keeper', 'user_session', 'session', 'campaign', $3,
            'write_official_state', 'human_keeper', 'party_visible',
            'not_applicable', 'rules_engine_decision', 'decision', 'rules_engine',
            'PERMIT', 'fga', 'fga_revision', 'opa', 'opa_revision', 'trace',
            $4, 1, $5, 'fixture_key', $6, $7
        ) RETURNING sequence
        "#,
    )
    .bind(sequence)
    .bind(commit_id)
    .bind(campaign_id)
    .bind(format!("sha256:{digest}"))
    .bind(format!("hmac-sha256:{digest}"))
    .bind(previous_hash)
    .bind(format!("hmac-sha256:{digest}"))
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn insert_pre_p03_event(
    pool: &PgPool,
    campaign_id: &str,
    idempotency_key: &str,
    stream_version: i64,
) -> i64 {
    sqlx::query_scalar(
        r#"
        INSERT INTO event_store (
            event_type, command_id, idempotency_key, expected_version,
            authority_mode, authority_contract_version, visibility_label,
            fact_provenance_kind, fact_provenance_reference, fact_recorded_by,
            correlation_id, causation_id, payload_json, campaign_id,
            stream_version, authenticated_actor_id, resource_type, resource_id,
            authority_contract_id, authority_owner, visibility_subject, trace_id,
            event_integrity_hash
        ) VALUES (
            'InterleavedUpgradeProbe', 'command', $2, 0, 'human_kp', 1,
            'party_visible', 'rules_engine_decision', 'fixture', 'rules_engine',
            'correlation', 'causation', '{}', $1, $3, 'keeper', 'campaign', $1,
            'authority_fixture', 'keeper', 'not_applicable', 'trace',
            'hmac-sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee'
        ) RETURNING sequence
        "#,
    )
    .bind(campaign_id)
    .bind(idempotency_key)
    .bind(stream_version)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn insert_pre_p03_outbox(
    pool: &PgPool,
    event_sequence: i64,
    idempotency_key: &str,
    commit_id: &str,
) {
    sqlx::query(
        r#"
        INSERT INTO event_outbox (
            event_sequence, nats_subject, idempotency_key, visibility_label,
            correlation_id, causation_id, payload_json, commit_id
        ) VALUES ($1, 'trpg.events.appended', $2, 'party_visible',
                  'correlation', 'causation', '{}', $3)
        "#,
    )
    .bind(event_sequence)
    .bind(idempotency_key)
    .bind(commit_id)
    .execute(pool)
    .await
    .unwrap();
}

async fn insert_pre_p03_formal(
    pool: &PgPool,
    commit_id: &str,
    campaign_id: &str,
    request_hash: &str,
    first_sequence: i64,
    last_sequence: i64,
    audit_sequence: i64,
) {
    let committed_at = if commit_id == "commit_a" {
        "2026-01-01T00:00:00Z"
    } else {
        "2026-01-02T00:00:00Z"
    };
    sqlx::query(
        r#"
        INSERT INTO formal_commits (
            commit_id, campaign_id, idempotency_key, request_hash,
            expected_version, first_event_sequence, last_event_sequence,
            first_stream_version, last_stream_version, audit_sequence,
            witness_prepare_sequence, witness_prepare_hash, committed_at
        ) VALUES ($1, $2, $1 || '_idem', $3, 0, $4, $5, 1, 2, $6, 1,
                  'hmac-sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc',
                  $7::timestamptz)
        "#,
    )
    .bind(commit_id)
    .bind(campaign_id)
    .bind(request_hash)
    .bind(first_sequence)
    .bind(last_sequence)
    .bind(audit_sequence)
    .bind(committed_at)
    .execute(pool)
    .await
    .unwrap();
}

fn schema_assertion_sql() -> String {
    include_str!("../../../scripts/ci/assert-schema.sql")
        .lines()
        .filter(|line| !line.trim_start().starts_with("\\set"))
        .collect::<Vec<_>>()
        .join("\n")
}

async fn assert_schema(pool: &PgPool) {
    sqlx::raw_sql(&schema_assertion_sql())
        .execute(pool)
        .await
        .expect("relation-qualified P03 schema and data invariants");
}

async fn assert_schema_rejects(pool: &PgPool, expected: &str) {
    let error = sqlx::raw_sql(&schema_assertion_sql())
        .execute(pool)
        .await
        .expect_err("drifted P03 schema must fail the machine assertion");
    assert!(
        error.to_string().contains(expected),
        "unexpected schema assertion error: {error}"
    );
}

#[derive(Clone, Copy)]
struct EventInsert<'a> {
    campaign_id: &'a str,
    stream_id: &'a str,
    stream_version: i64,
    idempotency_key: &'a str,
    operation: &'a str,
    expected_version: i64,
    authority_mode: &'a str,
    visibility_label: &'a str,
    provenance_kind: &'a str,
    event_schema_version: i32,
    payload_json: &'a str,
    payload_integrity_source: &'a str,
}

impl<'a> EventInsert<'a> {
    fn valid(campaign_id: &'a str, stream_id: &'a str) -> Self {
        Self {
            campaign_id,
            stream_id,
            stream_version: 1,
            idempotency_key: "shared_key",
            operation: "canonical_commit",
            expected_version: 0,
            authority_mode: "human_kp",
            visibility_label: "party_visible",
            provenance_kind: "rules_engine_decision",
            event_schema_version: CURRENT_EVENT_SCHEMA_VERSION,
            payload_json: "{}",
            payload_integrity_source: "{}",
        }
    }
}

async fn insert_event(pool: &PgPool, event: EventInsert<'_>) -> Result<PgQueryResult, sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO event_store (
            event_type, command_id, idempotency_key, expected_version,
            authority_mode, authority_contract_version, visibility_label,
            fact_provenance_kind, fact_provenance_reference, fact_recorded_by,
            correlation_id, causation_id, payload_json, campaign_id,
            stream_version, authenticated_actor_id, resource_type, resource_id,
            authority_contract_id, authority_owner, visibility_subject, trace_id,
            event_integrity_hash, stream_id, event_schema_version,
            idempotency_operation, request_hash, request_hash_source,
            integrity_status, payload_integrity_source
        ) VALUES (
            'MigrationUpgradeProbe', 'command_probe', $4, $6, $7, 1, $8,
            $9, 'fixture_reference', 'migration_upgrade', 'correlation_probe',
            'causation_probe', $11::jsonb, $1, $3, 'actor_probe', 'campaign',
            $1, 'authority_contract_probe', 'keeper_probe', 'not_applicable',
            'trace_probe',
            'hmac-sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd',
            $2, $10, $5, $12, 'formal_commit', 'verified_hmac', $13
        )
        "#,
    )
    .bind(event.campaign_id)
    .bind(event.stream_id)
    .bind(event.stream_version)
    .bind(event.idempotency_key)
    .bind(event.operation)
    .bind(event.expected_version)
    .bind(event.authority_mode)
    .bind(event.visibility_label)
    .bind(event.provenance_kind)
    .bind(event.event_schema_version)
    .bind(event.payload_json)
    .bind(REQUEST_HASH_A)
    .bind(event.payload_integrity_source)
    .execute(pool)
    .await
}

async fn try_insert_historical_probe_event(
    transaction: &mut Transaction<'_, Postgres>,
    idempotency_key: &str,
    stream_version: i64,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        INSERT INTO event_store (
            event_type, command_id, idempotency_key, expected_version,
            authority_mode, authority_contract_version, visibility_label,
            fact_provenance_kind, fact_provenance_reference, fact_recorded_by,
            correlation_id, causation_id, payload_json, campaign_id,
            stream_version, authenticated_actor_id, resource_type, resource_id,
            authority_contract_id, authority_owner, visibility_subject, trace_id,
            event_integrity_hash, stream_id, event_schema_version,
            idempotency_operation, request_hash, request_hash_source,
            integrity_status, payload_integrity_source
        ) VALUES (
            'HistoricalConstraintProbe', 'historical_probe_command', $1, 0,
            'human_kp', 1, 'party_visible', 'imported_source',
            'migration_constraint_probe', 'migration_upgrade',
            'correlation_probe', 'causation_probe', '{}'::jsonb,
            'historical_unscoped', $2, 'historical_unknown',
            'historical_unknown', 'historical_unknown', 'historical_unknown',
            'historical_unknown', 'not_applicable', 'historical_unknown', NULL,
            'historical_unscoped', 1, 'canonical_commit', $3,
            'historical_unavailable', 'historical_unsigned', '{}'
        ) RETURNING sequence
        "#,
    )
    .bind(idempotency_key)
    .bind(stream_version)
    .bind(ZERO_REQUEST_HASH)
    .fetch_one(&mut **transaction)
    .await
}

async fn try_insert_hmac_probe_event(
    transaction: &mut Transaction<'_, Postgres>,
    idempotency_key: &str,
    campaign_id: &str,
    stream_id: &str,
    integrity_status: &str,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        INSERT INTO event_store (
            event_type, command_id, idempotency_key, expected_version,
            authority_mode, authority_contract_version, visibility_label,
            fact_provenance_kind, fact_provenance_reference, fact_recorded_by,
            correlation_id, causation_id, payload_json, campaign_id,
            stream_version, authenticated_actor_id, resource_type, resource_id,
            authority_contract_id, authority_owner, visibility_subject, trace_id,
            event_integrity_hash, stream_id, event_schema_version,
            idempotency_operation, request_hash, request_hash_source,
            integrity_status, payload_integrity_source
        ) VALUES (
            'FormalConstraintProbe', 'formal_probe_command', $1, 0,
            'human_kp', 1, 'party_visible', 'rules_engine_decision',
            'migration_constraint_probe', 'migration_upgrade',
            'correlation_probe', 'causation_probe', '{}'::jsonb, $2, 1,
            'actor_probe', 'campaign', $2, 'authority_contract_probe',
            'keeper_probe', 'not_applicable', 'trace_probe',
            'hmac-sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd',
            $3, 1, 'canonical_commit', $4, 'formal_commit', $5, '{}'
        ) RETURNING sequence
        "#,
    )
    .bind(idempotency_key)
    .bind(campaign_id)
    .bind(stream_id)
    .bind(REQUEST_HASH_A)
    .bind(integrity_status)
    .fetch_one(&mut **transaction)
    .await
}

async fn insert_formal_probe_event(
    transaction: &mut Transaction<'_, Postgres>,
    idempotency_key: &str,
    campaign_id: &str,
    stream_id: &str,
) -> i64 {
    try_insert_hmac_probe_event(
        transaction,
        idempotency_key,
        campaign_id,
        stream_id,
        "verified_hmac",
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn migration_upgrade_covers_empty_b24_repeat_drift_and_constraints() {
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
    migrator_before_p03(current)
        .run(&pool)
        .await
        .expect("apply the canonical schema before audit-trigger drift probe");
    pool.execute("DROP TRIGGER canonical_audit_log_append_only ON canonical_audit_log")
        .await
        .unwrap();
    let audit_trigger_error = current
        .run(&pool)
        .await
        .expect_err("missing audit mutation guard must fail before P03");
    assert!(audit_trigger_error
        .to_string()
        .contains("mutation trigger drift"));
    assert_hardening_not_recorded(&pool).await;
    pool.close().await;
    pool = test_pool().await;

    reset_database(&pool).await;
    migrator_before_p03(current)
        .run(&pool)
        .await
        .expect("apply the canonical schema before audit-chain drift probe");
    sqlx::raw_sql(
        r#"
        CREATE OR REPLACE FUNCTION enforce_canonical_audit_chain()
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
    let audit_chain_error = current
        .run(&pool)
        .await
        .expect_err("audit-chain function drift must fail before P03");
    assert!(audit_chain_error
        .to_string()
        .contains("chain function drift"));
    assert_hardening_not_recorded(&pool).await;
    pool.close().await;
    pool = test_pool().await;

    // A formal commit's global sequence bounds may overlap another campaign's
    // bounds. Request hashes therefore have to follow the exact
    // event_outbox.commit_id binding, never a BETWEEN range over event_store.
    reset_database(&pool).await;
    migrator_before_p03(current)
        .run(&pool)
        .await
        .expect("apply the canonical schema immediately before P03");
    let a1 = insert_pre_p03_event(&pool, "campaign_interleave_a", "event_a_1", 1).await;
    let b1 = insert_pre_p03_event(&pool, "campaign_interleave_b", "event_b_1", 1).await;
    let b2 = insert_pre_p03_event(&pool, "campaign_interleave_b", "event_b_2", 2).await;
    let a2 = insert_pre_p03_event(&pool, "campaign_interleave_a", "event_a_2", 2).await;
    insert_pre_p03_outbox(&pool, a1, "outbox_a_1", "commit_a").await;
    insert_pre_p03_outbox(&pool, b1, "outbox_b_1", "commit_b").await;
    insert_pre_p03_outbox(&pool, b2, "outbox_b_2", "commit_b").await;
    insert_pre_p03_outbox(&pool, a2, "outbox_a_2", "commit_a").await;
    let audit_a = insert_pre_p03_audit(&pool, "commit_a", "campaign_interleave_a", 'a').await;
    let audit_b = insert_pre_p03_audit(&pool, "commit_b", "campaign_interleave_b", 'b').await;
    assert_eq!((audit_a, audit_b), (1, 2));
    insert_pre_p03_formal(
        &pool,
        "commit_a",
        "campaign_interleave_a",
        REQUEST_HASH_A,
        a1,
        a2,
        audit_a,
    )
    .await;
    insert_pre_p03_formal(
        &pool,
        "commit_b",
        "campaign_interleave_b",
        REQUEST_HASH_B,
        b1,
        b2,
        audit_b,
    )
    .await;
    current
        .run(&pool)
        .await
        .expect("interleaved pre-P03 data upgrades");
    let mismatched_request_hashes: i64 = sqlx::query_scalar(
        r#"
        SELECT count(*)
          FROM event_store AS event
          JOIN event_outbox AS outbox ON outbox.event_sequence = event.sequence
          JOIN formal_commits AS formal ON formal.commit_id = outbox.commit_id
         WHERE event.request_hash <> formal.request_hash
            OR outbox.request_hash <> formal.request_hash
        "#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        mismatched_request_hashes, 0,
        "P03 must not bind an interleaved event to another campaign's request"
    );
    let upgraded_integrity_states: Vec<(String, String)> = sqlx::query_as(
        r#"
        SELECT DISTINCT event.integrity_status, outbox.integrity_status
          FROM event_store AS event
          JOIN event_outbox AS outbox ON outbox.event_sequence = event.sequence
         WHERE event.event_integrity_hash IS NOT NULL
         ORDER BY event.integrity_status, outbox.integrity_status
        "#,
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        upgraded_integrity_states,
        vec![(
            "historical_unverified_hmac".to_owned(),
            "historical_unverified_hmac".to_owned()
        )],
        "migration must not claim cryptographic verification it did not perform"
    );
    let audit_integrity_versions: Vec<i32> =
        sqlx::query_scalar("SELECT DISTINCT integrity_version FROM canonical_audit_log")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(audit_integrity_versions, vec![1]);
    let audit_occurred_at_before: String =
        sqlx::query_scalar("SELECT occurred_at::text FROM canonical_audit_log WHERE sequence = $1")
            .bind(audit_a)
            .fetch_one(&pool)
            .await
            .unwrap();
    let audit_update_error = sqlx::query(
        "UPDATE canonical_audit_log SET occurred_at = occurred_at + interval '1 second' WHERE sequence = $1",
    )
    .bind(audit_a)
    .execute(&pool)
    .await
    .expect_err("audit timestamp must remain append-only after upgrade");
    assert!(audit_update_error
        .to_string()
        .contains("canonical commit records are append-only"));
    let audit_occurred_at_after: String =
        sqlx::query_scalar("SELECT occurred_at::text FROM canonical_audit_log WHERE sequence = $1")
            .bind(audit_a)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(audit_occurred_at_before, audit_occurred_at_after);
    assert!(
        sqlx::query("DELETE FROM canonical_audit_log WHERE sequence = $1")
            .bind(audit_a)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(sqlx::query("TRUNCATE canonical_audit_log")
        .execute(&pool)
        .await
        .is_err());

    // A committed marker freezes its exact Event/Outbox set. Reusing the
    // existing commit_id for a later pair must fail at deferred validation,
    // even when campaign, stream, request hash, and per-row metadata match.
    let mut commit_reuse_transaction = pool.begin().await.unwrap();
    let reused_sequence: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO event_store (
            event_type, command_id, idempotency_key, expected_version,
            authority_mode, authority_contract_version, visibility_label,
            fact_provenance_kind, fact_provenance_reference, fact_recorded_by,
            correlation_id, causation_id, payload_json, campaign_id,
            stream_version, authenticated_actor_id, resource_type, resource_id,
            authority_contract_id, authority_owner, visibility_subject, trace_id,
            event_integrity_hash, stream_id, event_schema_version,
            idempotency_operation, request_hash, request_hash_source,
            integrity_status, payload_integrity_source
        ) VALUES (
            'CommitReuseProbe', 'commit_reuse_command', 'commit_reuse_event', 2,
            'human_kp', 1, 'party_visible', 'rules_engine_decision',
            'commit_reuse_probe', 'migration_upgrade', 'commit_reuse_correlation',
            'commit_reuse_causation', '{}'::jsonb, 'campaign_interleave_a', 3,
            'keeper', 'campaign', 'campaign_interleave_a', 'authority_fixture',
            'keeper', 'not_applicable', 'commit_reuse_trace',
            'hmac-sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd',
            'campaign_interleave_a', 1, 'canonical_commit', $1,
            'formal_commit', 'verified_hmac', '{}'
        ) RETURNING sequence
        "#,
    )
    .bind(REQUEST_HASH_A)
    .fetch_one(&mut *commit_reuse_transaction)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO event_outbox (
            event_id, event_sequence, nats_subject, idempotency_key,
            visibility_label, correlation_id, causation_id, payload_json,
            commit_id, campaign_id, stream_id, event_schema_version,
            idempotency_operation, request_hash, request_hash_source,
            integrity_status
        ) VALUES (
            $1, $1, 'trpg.events.appended', 'commit_reuse_outbox',
            'party_visible', 'commit_reuse_correlation',
            'commit_reuse_causation', '{}'::jsonb, 'commit_a',
            'campaign_interleave_a', 'campaign_interleave_a', 1,
            'canonical_commit', $2, 'formal_commit', 'verified_hmac'
        )
        "#,
    )
    .bind(reused_sequence)
    .bind(REQUEST_HASH_A)
    .execute(&mut *commit_reuse_transaction)
    .await
    .unwrap();
    let commit_reuse_error = sqlx::query("SET CONSTRAINTS ALL IMMEDIATE")
        .execute(&mut *commit_reuse_transaction)
        .await
        .expect_err("a committed marker cannot acquire a later Event/Outbox pair");
    assert!(commit_reuse_error
        .to_string()
        .contains("formal commit exact event/outbox set changed after commit"));
    commit_reuse_transaction.rollback().await.unwrap();

    // A genuine b-24 SQLx ledger and data set upgrades without checksum edits.
    reset_database(&pool).await;
    b24.run(&pool)
        .await
        .expect("apply historical b-24 migration");

    let legacy_sequence: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO event_store (
            event_type, command_id, idempotency_key, expected_version,
            authority_mode, authority_contract_version, visibility_label,
            fact_provenance_kind, fact_provenance_reference, fact_recorded_by,
            correlation_id, causation_id, payload_json
        ) VALUES (
            'CampaignStarted', 'legacy_command', 'legacy_idem:0000', 0,
            'human_kp', 1, 'party_visible', 'rules_engine_decision',
            'legacy_reference', 'legacy_keeper', 'legacy_correlation',
            'legacy_causation', '{"legacy":true}'
        ) RETURNING sequence
        "#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO event_outbox (
            event_sequence, nats_subject, idempotency_key, visibility_label,
            correlation_id, causation_id, payload_json
        ) VALUES ($1, 'trpg.events.appended', 'legacy_outbox:0000',
                  'party_visible', 'legacy_correlation', 'legacy_causation',
                  '{"legacy":true}')
        "#,
    )
    .bind(legacy_sequence)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO projection_checkpoint (projection_name, last_event_sequence, projection_hash) VALUES ('legacy_projection', $1, 'legacy_hash')",
    )
    .bind(legacy_sequence)
    .execute(&pool)
    .await
    .unwrap();

    current
        .run(&pool)
        .await
        .expect("b-24 upgrades without VersionMismatch");
    assert_schema(&pool).await;

    let event: EventStoreRecord = sqlx::query_as("SELECT * FROM event_store WHERE sequence = $1")
        .bind(legacy_sequence)
        .fetch_one(&pool)
        .await
        .expect("lossless SQLx event mapping");
    assert_eq!(event.payload_json, json!({"legacy": true}));
    assert_eq!(event.campaign_id, "historical_unscoped");
    assert_eq!(event.stream_id, "historical_unscoped");
    assert_eq!(event.event_schema_version, 1);
    assert_eq!(event.request_hash_source, "historical_unavailable");
    assert_eq!(event.integrity_status, "historical_unsigned");
    assert_eq!(event.event_integrity_hash, None);
    assert_eq!(event.payload_integrity_source, r#"{"legacy":true}"#);
    let serde_round_trip: EventStoreRecord =
        serde_json::from_value(serde_json::to_value(&event).unwrap()).unwrap();
    assert_eq!(serde_round_trip, event);

    let outbox: EventOutboxRecord =
        sqlx::query_as("SELECT * FROM event_outbox WHERE event_sequence = $1")
            .bind(legacy_sequence)
            .fetch_one(&pool)
            .await
            .expect("lossless SQLx outbox mapping");
    assert_eq!(outbox.event_id, legacy_sequence);
    assert_eq!(outbox.payload_json, json!({"legacy": true}));
    assert_eq!(outbox.request_hash_source, "historical_unavailable");
    assert_eq!(outbox.integrity_status, "historical_unsigned");
    let outbox_serde_round_trip: EventOutboxRecord =
        serde_json::from_value(serde_json::to_value(&outbox).unwrap()).unwrap();
    assert_eq!(outbox_serde_round_trip, outbox);
    assert!(
        sqlx::query("DELETE FROM event_outbox WHERE event_sequence = $1")
            .bind(legacy_sequence)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(sqlx::query("TRUNCATE event_outbox")
        .execute(&pool)
        .await
        .is_err());
    let mut delivery_state_transaction = pool.begin().await.unwrap();
    sqlx::query(
        "UPDATE event_outbox SET claimed_at = now(), claim_owner = 'p03_probe', retry_count = retry_count + 1 WHERE event_sequence = $1",
    )
    .bind(legacy_sequence)
    .execute(&mut *delivery_state_transaction)
    .await
    .expect("delivery state remains mutable");
    delivery_state_transaction.rollback().await.unwrap();
    for identity_mutation in [
        "UPDATE event_outbox SET outbox_id = outbox_id + 1000 WHERE event_sequence = $1",
        "UPDATE event_outbox SET idempotency_key = idempotency_key || ':rebound' WHERE event_sequence = $1",
        "UPDATE event_outbox SET commit_id = 'rebound_commit' WHERE event_sequence = $1",
    ] {
        let error = sqlx::query(identity_mutation)
            .bind(legacy_sequence)
            .execute(&pool)
            .await
            .expect_err("canonical outbox identity must be immutable");
        assert!(error
            .to_string()
            .contains("canonical outbox identity is immutable"));
    }

    let checkpoint: ProjectionCheckpointRecord = sqlx::query_as(
        "SELECT * FROM projection_checkpoint WHERE projection_name = 'legacy_projection'",
    )
    .fetch_one(&pool)
    .await
    .expect("lossless SQLx checkpoint mapping");
    assert_eq!(checkpoint.version, legacy_sequence);
    assert_eq!(checkpoint.stream_id, "legacy_projection");
    let checkpoint_serde_round_trip: ProjectionCheckpointRecord =
        serde_json::from_value(serde_json::to_value(&checkpoint).unwrap()).unwrap();
    assert_eq!(checkpoint_serde_round_trip, checkpoint);

    let upcasted = EventPayloadUpcaster::canonical()
        .upcast(
            &event.event_type,
            event.event_schema_version,
            event.payload_json,
        )
        .expect("known historical version upcasts");
    assert_eq!(upcasted.event_schema_version, CURRENT_EVENT_SCHEMA_VERSION);
    assert_eq!(upcasted.payload, json!({"legacy": true}));
    assert!(EventPayloadUpcaster::canonical()
        .upcast("CampaignStarted", 99, Value::Null)
        .is_err());

    let replayed = load_canonical_replay_page(&pool, "historical_unscoped", 0, 100)
        .await
        .expect("production replay path accepts explicitly unsigned historical data");
    let replayed_legacy = replayed
        .iter()
        .find(|candidate| candidate.sequence == legacy_sequence)
        .expect("upgraded b-24 event is returned by production replay");
    assert_eq!(replayed_legacy.payload, json!({"legacy": true}));
    assert_eq!(replayed_legacy.event_integrity_hash, None);
    assert_eq!(
        replayed_legacy.request_hash_source,
        "historical_unavailable"
    );
    assert_eq!(replayed_legacy.integrity_status, "historical_unsigned");

    // Invalid JSON, enum, negative version, and blank IDs are rejected by the
    // database. Scoped idempotency is covered by the exact constraint
    // signature here and by the real canonical-commit integration test.
    let mixed_integrity_metadata = sqlx::query(
        r#"
        INSERT INTO event_store (
            event_type, command_id, idempotency_key, expected_version,
            authority_mode, authority_contract_version, visibility_label,
            fact_provenance_kind, fact_provenance_reference, fact_recorded_by,
            correlation_id, causation_id, payload_json, campaign_id,
            stream_version, authenticated_actor_id, resource_type, resource_id,
            authority_contract_id, authority_owner, visibility_subject, trace_id,
            event_integrity_hash, stream_id, event_schema_version,
            idempotency_operation, request_hash, request_hash_source,
            integrity_status, payload_integrity_source
        ) VALUES (
            'MixedIntegrityProbe', 'mixed_integrity_command',
            'mixed_integrity_event', 0, 'human_kp', 1, 'party_visible',
            'imported_source', 'migration_constraint_probe', 'migration_upgrade',
            'mixed_integrity_correlation', 'mixed_integrity_causation', '{}'::jsonb,
            'historical_unscoped', $1, 'historical_unknown', 'historical_unknown',
            'historical_unknown', 'historical_unknown', 'historical_unknown',
            'not_applicable', 'historical_unknown',
            'hmac-sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd',
            'historical_unscoped', 1, 'canonical_commit', $2,
            'historical_unavailable', 'verified_hmac', '{}'
        )
        "#,
    )
    .bind(legacy_sequence + 1)
    .bind(ZERO_REQUEST_HASH)
    .execute(&pool)
    .await;
    assert!(mixed_integrity_metadata.is_err());
    assert!(insert_event(
        &pool,
        EventInsert {
            payload_json: "not-json",
            ..EventInsert::valid("campaign_invalid_json", "stream_invalid_json")
        },
    )
    .await
    .is_err());
    assert!(insert_event(
        &pool,
        EventInsert {
            authority_mode: "forged_kp",
            ..EventInsert::valid("campaign_invalid_enum", "stream_invalid_enum")
        },
    )
    .await
    .is_err());
    assert!(insert_event(
        &pool,
        EventInsert {
            expected_version: -1,
            ..EventInsert::valid("campaign_negative", "stream_negative")
        },
    )
    .await
    .is_err());
    assert!(insert_event(
        &pool,
        EventInsert {
            visibility_label: "all_players_and_keeper_secrets",
            ..EventInsert::valid("campaign_invalid_visibility", "stream_invalid_visibility")
        },
    )
    .await
    .is_err());
    assert!(insert_event(
        &pool,
        EventInsert {
            provenance_kind: "invented_by_agent",
            ..EventInsert::valid("campaign_invalid_provenance", "stream_invalid_provenance")
        },
    )
    .await
    .is_err());
    assert!(insert_event(
        &pool,
        EventInsert {
            payload_integrity_source: r#"{"different":true}"#,
            ..EventInsert::valid("campaign_payload_mismatch", "stream_payload_mismatch")
        },
    )
    .await
    .is_err());
    assert!(
        insert_event(&pool, EventInsert::valid("campaign_blank", " "))
            .await
            .is_err()
    );
    let orphan_error = insert_event(
        &pool,
        EventInsert::valid("campaign_orphan_current", "stream_orphan_current"),
    )
    .await
    .expect_err("a current event cannot commit without its outbox/formal marker");
    assert!(orphan_error
        .to_string()
        .contains("formal event lacks one complete outbox/commit binding"));

    // Historical classification is migration output, not an application
    // write mode. A post-HEAD Event + Outbox pair used to bypass the formal
    // commit and HMAC invariants; reject both insertion points while retaining
    // the genuine b-24 rows that were classified during the migration above.
    let mut historical_event_transaction = pool.begin().await.unwrap();
    let historical_event_error = try_insert_historical_probe_event(
        &mut historical_event_transaction,
        "post_head_historical_event",
        legacy_sequence + 1,
    )
    .await
    .expect_err("post-HEAD historical events must be rejected at insertion");
    assert!(historical_event_error
        .to_string()
        .contains("historical classification is migration-only"));
    historical_event_transaction.rollback().await.unwrap();

    let mut unverified_hmac_transaction = pool.begin().await.unwrap();
    let unverified_hmac_error = try_insert_hmac_probe_event(
        &mut unverified_hmac_transaction,
        "post_head_unverified_hmac_event",
        "post_head_unverified_hmac_campaign",
        "post_head_unverified_hmac_stream",
        "historical_unverified_hmac",
    )
    .await
    .expect_err("post-HEAD unverified historical HMAC events must be rejected at insertion");
    assert!(unverified_hmac_error
        .to_string()
        .contains("historical classification is migration-only"));
    unverified_hmac_transaction.rollback().await.unwrap();

    let historical_outbox_error = sqlx::query(
        r#"
        INSERT INTO event_outbox (
            event_id, event_sequence, nats_subject, idempotency_key,
            visibility_label, correlation_id, causation_id, payload_json,
            retry_count, campaign_id, stream_id, event_schema_version,
            idempotency_operation, request_hash, request_hash_source,
            integrity_status
        ) VALUES (
            $1, $1, 'trpg.events.appended', 'post_head_historical_outbox',
            'party_visible', 'legacy_correlation', 'legacy_causation',
            '{"legacy":true}'::jsonb, 0, 'historical_unscoped',
            'historical_unscoped', 1, 'canonical_commit', $2,
            'historical_unavailable', 'historical_unsigned'
        )
        "#,
    )
    .bind(legacy_sequence)
    .bind(ZERO_REQUEST_HASH)
    .execute(&pool)
    .await
    .expect_err("post-HEAD historical outboxes must be rejected at insertion");
    assert!(historical_outbox_error
        .to_string()
        .contains("historical classification is migration-only"));

    let bad_reference = sqlx::query(
        r#"
        INSERT INTO event_outbox (
            event_id, event_sequence, nats_subject, idempotency_key,
            visibility_label, correlation_id, causation_id, payload_json,
            retry_count, campaign_id, stream_id, event_schema_version,
            idempotency_operation, request_hash, request_hash_source,
            integrity_status
        ) VALUES (
            999999999, 999999999, 'trpg.events.appended', 'bad_reference',
            'party_visible', 'correlation', 'causation', '{}'::jsonb, 0,
            'missing_campaign', 'missing_stream', 1,
            'canonical_commit', $1, 'formal_commit', 'verified_hmac'
        )
        "#,
    )
    .bind(REQUEST_HASH_A)
    .execute(&pool)
    .await;
    assert!(bad_reference.is_err());

    let mut negative_retry_transaction = pool.begin().await.unwrap();
    let negative_retry_event = insert_formal_probe_event(
        &mut negative_retry_transaction,
        "negative_retry_event",
        "negative_retry_campaign",
        "negative_retry_stream",
    )
    .await;
    let negative_retry = sqlx::query(
        r#"
        INSERT INTO event_outbox (
            event_id, event_sequence, nats_subject, idempotency_key,
            visibility_label, correlation_id, causation_id, payload_json,
            retry_count, campaign_id, stream_id, event_schema_version,
            idempotency_operation, request_hash, request_hash_source,
            integrity_status
        ) VALUES (
            $1, $1, 'trpg.events.appended', 'negative_retry',
            'party_visible', 'correlation_probe', 'causation_probe', '{}'::jsonb, -1,
            'negative_retry_campaign', 'negative_retry_stream', 1,
            'canonical_commit', $2, 'formal_commit', 'verified_hmac'
        )
        "#,
    )
    .bind(negative_retry_event)
    .bind(REQUEST_HASH_A)
    .execute(&mut *negative_retry_transaction)
    .await;
    assert!(negative_retry.is_err());
    negative_retry_transaction.rollback().await.unwrap();

    let mut invalid_subject_transaction = pool.begin().await.unwrap();
    let invalid_subject_event = insert_formal_probe_event(
        &mut invalid_subject_transaction,
        "invalid_subject_event",
        "invalid_subject_campaign",
        "invalid_subject_stream",
    )
    .await;
    let invalid_subject = sqlx::query(
        r#"
        INSERT INTO event_outbox (
            event_id, event_sequence, nats_subject, idempotency_key,
            visibility_label, correlation_id, causation_id, payload_json,
            retry_count, campaign_id, stream_id, event_schema_version,
            idempotency_operation, request_hash, request_hash_source,
            integrity_status
        ) VALUES (
            $1, $1, 'trpg.events.forged', 'invalid_subject',
            'party_visible', 'correlation_probe', 'causation_probe', '{}'::jsonb, 0,
            'invalid_subject_campaign', 'invalid_subject_stream', 1,
            'canonical_commit', $2, 'formal_commit', 'verified_hmac'
        )
        "#,
    )
    .bind(invalid_subject_event)
    .bind(REQUEST_HASH_A)
    .execute(&mut *invalid_subject_transaction)
    .await;
    assert!(invalid_subject.is_err());
    invalid_subject_transaction.rollback().await.unwrap();

    let mut mismatched_outbox_transaction = pool.begin().await.unwrap();
    let mismatched_outbox_event = insert_formal_probe_event(
        &mut mismatched_outbox_transaction,
        "mismatched_outbox_event",
        "mismatched_outbox_campaign",
        "mismatched_outbox_stream",
    )
    .await;
    let mismatched_outbox = sqlx::query(
        r#"
        INSERT INTO event_outbox (
            event_id, event_sequence, nats_subject, idempotency_key,
            visibility_label, correlation_id, causation_id, payload_json,
            retry_count, campaign_id, stream_id, event_schema_version,
            idempotency_operation, request_hash, request_hash_source,
            integrity_status
        ) VALUES (
            $1, $1, 'trpg.events.appended', 'mismatched_outbox',
            'keeper_only', 'correlation_probe', 'causation_probe', '{}'::jsonb, 0,
            'mismatched_outbox_campaign', 'mismatched_outbox_stream', 1,
            'canonical_commit', $2, 'formal_commit', 'verified_hmac'
        )
        "#,
    )
    .bind(mismatched_outbox_event)
    .bind(REQUEST_HASH_A)
    .execute(&mut *mismatched_outbox_transaction)
    .await;
    assert!(mismatched_outbox.is_err());
    mismatched_outbox_transaction.rollback().await.unwrap();

    let ledger_before_repeat: i64 = sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .unwrap();
    current
        .run(&pool)
        .await
        .expect("upgraded database repeat no-op");
    let ledger_after_repeat: i64 = sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(ledger_before_repeat, ledger_after_repeat);
}
