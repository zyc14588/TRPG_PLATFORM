use std::borrow::Cow;
use std::env;
use std::path::Path;
use std::str::FromStr;

use serde_json::{json, Value};
use sqlx::migrate::Migrator;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions, PgQueryResult};
use sqlx::{Executor, PgPool, Postgres, Transaction};
use trpg_data_eventing::event_store_sqlx_outbox_projection::load_canonical_replay_page;
use trpg_data_eventing::event_store_sqlx_outbox_projection::PayloadCipher;
use trpg_data_eventing::outbox_projection_workers::PostgresProjectionWorker;
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
const PROTECTED_PAYLOAD_FIXTURE: &str = r#"{"protected_payload":{"algorithm":"AES-256-GCM","key_reference":"migration_fixture_key","nonce":"AAAAAAAAAAAAAAAA","ciphertext":"AAAAAAAAAAAAAAAAAAAAAA=="}}"#;

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
        "DROP SCHEMA IF EXISTS core_domain CASCADE; \
         DROP SCHEMA public CASCADE; \
         CREATE SCHEMA public; \
         GRANT ALL ON SCHEMA public TO public; \
         CREATE EXTENSION IF NOT EXISTS vector;",
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

fn migrator_through(current: &Migrator, maximum_version: i64) -> Migrator {
    Migrator {
        migrations: Cow::Owned(
            current
                .iter()
                .filter(|migration| migration.version <= maximum_version)
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

async fn assert_schema(pool: &PgPool) {
    sqlx::raw_sql(&schema_assertion_sql())
        .execute(pool)
        .await
        .expect("relation-qualified P03 schema and data invariants");
}

async fn assert_schema_rejects(pool: &PgPool, expected: &str) {
    let mut connection = pool
        .acquire()
        .await
        .expect("acquire dedicated schema-assertion connection");
    let error = sqlx::raw_sql(&schema_assertion_sql())
        .execute(&mut *connection)
        .await
        .expect_err("drifted P03 schema must fail the machine assertion");
    sqlx::query("ROLLBACK")
        .execute(&mut *connection)
        .await
        .expect("rollback failed schema assertion transaction");
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
            payload_json: PROTECTED_PAYLOAD_FIXTURE,
            payload_integrity_source: PROTECTED_PAYLOAD_FIXTURE,
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
            stream_version, authenticated_actor_id, authenticated_actor_role,
            authenticated_actor_origin, resource_type, resource_id,
            authority_contract_id, authority_owner, visibility_subject, trace_id,
            event_integrity_hash, stream_id, event_schema_version,
            idempotency_operation, request_hash, request_hash_source,
            integrity_status, payload_integrity_source, payload_ciphertext,
            payload_key_reference, payload_nonce
        ) VALUES (
            'MigrationUpgradeProbe', 'command_probe', $4, $6, $7, 1, $8,
            $9, 'fixture_reference', 'migration_upgrade', 'correlation_probe',
            'causation_probe', $11::jsonb, $1, $3, 'actor_probe', 'workflow',
            '{"kind":"workload","role":"workflow_engine"}'::jsonb, 'campaign',
            $1, 'authority_contract_probe', 'keeper_probe', 'not_applicable',
            'trace_probe',
            'hmac-sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd',
            $2, $10, $5, $12, 'formal_commit', 'verified_hmac', $13,
            decode(repeat('00', 16), 'hex'), 'migration_fixture_key',
            decode(repeat('00', 12), 'hex')
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
