use std::env;
use std::path::Path;
use std::str::FromStr;

use sqlx::migrate::Migrator;
use sqlx::postgres::PgConnectOptions;
use sqlx::PgPool;
use trpg_data_eventing::cache_redis_impl::{ProjectionCacheEntry, RedisProjectionCache};
use trpg_data_eventing::event_bus_nats_impl::JetStreamOutboxPublisher;
use trpg_data_eventing::event_store_sqlx_outbox_projection::{
    AtomicCommitDraft, CanonicalEventDraft, PolicyAuditDraft, PostgresCanonicalStore,
};

const KEY: &[u8; 32] = &[0xa7; 32];
const CORRUPT_ROW_REQUEST_HASH: &str =
    "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

async fn reset_dedicated_database(
    database_url: &str,
    authorized_database_variable: &str,
    install_vector: bool,
) -> PgPool {
    assert_eq!(
        env::var("P02_EVENTING_ALLOW_DATABASE_RESET").as_deref(),
        Ok("1"),
        "set P02_EVENTING_ALLOW_DATABASE_RESET=1 for the dedicated eventing integration database"
    );
    let options = PgConnectOptions::from_str(database_url).expect("valid eventing PostgreSQL URL");
    let host = options.get_host();
    let database = options.get_database().unwrap_or_default();
    let authorized_database = env::var(authorized_database_variable).unwrap_or_else(|_| {
        panic!("{authorized_database_variable} must name the dedicated database")
    });
    assert!(
        matches!(host, "localhost" | "127.0.0.1" | "::1") && database == authorized_database,
        "eventing upgrade test refuses to reset a non-dedicated local database"
    );
    let pool = PgPool::connect_with(options)
        .await
        .expect("connect to dedicated eventing integration database");
    let reset_sql = if install_vector {
        "DROP SCHEMA public CASCADE; CREATE SCHEMA public; GRANT ALL ON SCHEMA public TO public; CREATE EXTENSION IF NOT EXISTS vector;"
    } else {
        "DROP SCHEMA public CASCADE; CREATE SCHEMA public; GRANT ALL ON SCHEMA public TO public;"
    };
    sqlx::raw_sql(reset_sql)
        .execute(&pool)
        .await
        .expect("reset dedicated eventing integration database");
    pool
}

async fn reset_to_frozen_event_store(database_url: &str, witness_url: &str) -> PgPool {
    let pool = reset_dedicated_database(database_url, "P02_EVENTING_RESET_DATABASE", true).await;
    let witness_pool =
        reset_dedicated_database(witness_url, "P02_EVENTING_WITNESS_RESET_DATABASE", false).await;
    witness_pool.close().await;
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/b24");
    Migrator::new(fixture_path.as_path())
        .await
        .expect("resolve frozen event-store fixture")
        .run(&pool)
        .await
        .expect("apply frozen event-store fixture");
    pool
}

fn draft(suffix: u32) -> AtomicCommitDraft {
    let commit_id = format!("jetstream_commit_{suffix}");
    AtomicCommitDraft {
        commit_id: commit_id.clone(),
        campaign_id: format!("jetstream_campaign_{suffix}"),
        stream_id: format!("jetstream_campaign_{suffix}"),
        idempotency_key: format!("jetstream_idempotency_{suffix}"),
        expected_version: 0,
        command_id: format!("jetstream_command_{suffix}"),
        authenticated_actor_id: "workflow_jetstream".to_owned(),
        authority_mode: "human_kp".to_owned(),
        authority_contract_version: 1,
        authority_contract_id: format!("jetstream_authority_{suffix}"),
        authority_owner: "keeper_jetstream".to_owned(),
        visibility_label: "keeper_only".to_owned(),
        visibility_subject: "not_applicable".to_owned(),
        provenance_kind: "rules_engine_decision".to_owned(),
        provenance_reference: format!("jetstream_decision_{suffix}"),
        provenance_recorded_by: "rules_engine_jetstream".to_owned(),
        correlation_id: format!("jetstream_correlation_{suffix}"),
        causation_id: format!("jetstream_causation_{suffix}"),
        trace_id: format!("jetstream_trace_{suffix}"),
        events: vec![CanonicalEventDraft {
            event_type: "ClueDiscovered".to_owned(),
            payload_json: r#"{"clue":"harbor ledger"}"#.to_owned(),
        }],
        audit: PolicyAuditDraft {
            actor_id: "keeper_jetstream".to_owned(),
            actor_origin: "user_session".to_owned(),
            authentication_reference: "session_jetstream".to_owned(),
            resource_type: "campaign".to_owned(),
            resource_id: format!("jetstream_campaign_{suffix}"),
            action: "write_official_state".to_owned(),
            requested_role: "human_keeper".to_owned(),
            openfga_decision_id: format!("fga_jetstream_{suffix}"),
            openfga_policy_revision: "fga_jetstream_model".to_owned(),
            opa_decision_id: format!("opa_jetstream_{suffix}"),
            opa_policy_revision: "opa_jetstream_bundle".to_owned(),
        },
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn outbox_waits_for_jetstream_ack_and_redis_remains_a_versioned_read_model() {
    let database_url = env::var("P02_EVENTING_DATABASE_URL")
        .expect("P02_EVENTING_DATABASE_URL is required for the real PostgreSQL gate");
    let witness_url = env::var("P02_EVENTING_WITNESS_DATABASE_URL")
        .expect("P02_EVENTING_WITNESS_DATABASE_URL is required for the real PostgreSQL gate");
    let nats_url =
        env::var("P02_NATS_URL").expect("P02_NATS_URL is required for the real JetStream gate");
    let redis_url =
        env::var("P02_REDIS_URL").expect("P02_REDIS_URL is required for the real Redis gate");
    let suffix = std::process::id();

    // Seed a genuine pending row under the frozen schema. The HEAD migration,
    // rather than test SQL, is solely responsible for assigning its explicit
    // historical classification.
    let pool = reset_to_frozen_event_store(&database_url, &witness_url).await;
    let legacy_sequence: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO event_store (
            event_type, command_id, idempotency_key, expected_version,
            authority_mode, authority_contract_version, visibility_label,
            fact_provenance_kind, fact_provenance_reference, fact_recorded_by,
            correlation_id, causation_id, payload_json
        ) VALUES (
            'ClueDiscovered', $1, $2, 0, 'human_kp', 1, 'keeper_only',
            'imported_source', 'frozen_schema_upgrade_fixture',
            'migration_upgrade', $3, $4, '{"clue":"harbor ledger"}'
        ) RETURNING sequence
        "#,
    )
    .bind(format!("upgrade_command_{suffix}"))
    .bind(format!("upgrade_event_{suffix}"))
    .bind(format!("upgrade_correlation_{suffix}"))
    .bind(format!("upgrade_causation_{suffix}"))
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO event_outbox (
            event_sequence, nats_subject, idempotency_key, visibility_label,
            correlation_id, causation_id, payload_json
        ) VALUES (
            $1, 'trpg.events.appended', $2, 'keeper_only', $3, $4,
            '{"clue":"harbor ledger"}'
        )
        "#,
    )
    .bind(legacy_sequence)
    .bind(format!("upgrade_outbox_{suffix}"))
    .bind(format!("upgrade_correlation_{suffix}"))
    .bind(format!("upgrade_causation_{suffix}"))
    .execute(&pool)
    .await
    .unwrap();

    // A second frozen-schema row carries CR/LF in values that become NATS
    // headers. Old deployments allowed these nonblank strings. HEAD must keep
    // the publisher alive, fail only this delivery, and continue the batch.
    let poisoned_header_sequence: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO event_store (
            event_type, command_id, idempotency_key, expected_version,
            authority_mode, authority_contract_version, visibility_label,
            fact_provenance_kind, fact_provenance_reference, fact_recorded_by,
            correlation_id, causation_id, payload_json
        ) VALUES (
            'ClueDiscovered', $1, $2, 0, 'human_kp', 1, 'keeper_only',
            'imported_source', 'frozen_header_upgrade_fixture',
            'migration_upgrade', $3, $4, '{"clue":"poisoned header"}'
        ) RETURNING sequence
        "#,
    )
    .bind(format!("poisoned_header_command_{suffix}"))
    .bind(format!("poisoned_header_event_{suffix}"))
    .bind(format!("poisoned\r\ncorrelation_{suffix}"))
    .bind(format!("poisoned_header_causation_{suffix}"))
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO event_outbox (
            event_sequence, nats_subject, idempotency_key, visibility_label,
            correlation_id, causation_id, payload_json
        ) VALUES (
            $1, 'trpg.events.appended', $2, 'keeper_only', $3, $4,
            '{"clue":"poisoned header"}'
        )
        "#,
    )
    .bind(poisoned_header_sequence)
    .bind(format!("poisoned\r\nmessage_id_{suffix}"))
    .bind(format!("poisoned\r\ncorrelation_{suffix}"))
    .bind(format!("poisoned_header_causation_{suffix}"))
    .execute(&pool)
    .await
    .unwrap();

    let store =
        PostgresCanonicalStore::connect(&database_url, &witness_url, "p02-jetstream-key", KEY)
            .await
            .unwrap();
    store.prepare_for_service().await.unwrap();
    store.commit(&draft(suffix)).await.unwrap();

    // JetStream de-duplication is global to its NATS stream, while command
    // idempotency is scoped to campaign/resource stream. Both rows must be
    // published even though they intentionally reuse the same client key.
    let mut scoped_a = draft(suffix.saturating_add(1));
    scoped_a.commit_id = format!("jetstream_scoped_a_{suffix}");
    scoped_a.campaign_id = format!("jetstream_multistream_{suffix}");
    scoped_a.stream_id = format!("jetstream_scene_a_{suffix}");
    scoped_a.idempotency_key = format!("jetstream_shared_key_{suffix}");
    scoped_a.command_id = format!("jetstream_scoped_command_a_{suffix}");
    scoped_a.authority_contract_id = format!("jetstream_authority_multistream_{suffix}");
    scoped_a.audit.resource_type = "scene".to_owned();
    scoped_a.audit.resource_id = scoped_a.stream_id.clone();
    let mut scoped_b = draft(suffix.saturating_add(2));
    scoped_b.commit_id = format!("jetstream_scoped_b_{suffix}");
    scoped_b.campaign_id = scoped_a.campaign_id.clone();
    scoped_b.stream_id = format!("jetstream_scene_b_{suffix}");
    scoped_b.idempotency_key = scoped_a.idempotency_key.clone();
    scoped_b.command_id = format!("jetstream_scoped_command_b_{suffix}");
    scoped_b.authority_contract_id = scoped_a.authority_contract_id.clone();
    scoped_b.audit.resource_type = "scene".to_owned();
    scoped_b.audit.resource_id = scoped_b.stream_id.clone();
    store.commit(&scoped_a).await.unwrap();
    store.commit(&scoped_b).await.unwrap();

    // Simulate one recoverable storage-corruption row without weakening the
    // schema: PostgreSQL replication restore mode bypasses triggers only for
    // this dedicated test transaction. The row is internally formal/HMAC but
    // deliberately lacks its formal commit marker. It must fail independently
    // after claiming, while the two legitimate rows in the batch still publish.
    let mut corruption_transaction = pool.begin().await.unwrap();
    sqlx::query("SET LOCAL session_replication_role = replica")
        .execute(&mut *corruption_transaction)
        .await
        .unwrap();
    let corrupt_sequence: i64 = sqlx::query_scalar(
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
            'OutboxRecoveryProbe', $1, $2, 0, 'human_kp', 1, 'keeper_only',
            'rules_engine_decision', 'outbox_recovery_probe',
            'rules_engine_jetstream', $3, $4, '{"probe":"corrupt"}'::jsonb,
            $5, 1, 'workflow_jetstream', 'campaign', $5, $6,
            'keeper_jetstream', 'not_applicable', $7,
            'hmac-sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd',
            $8, 1, 'canonical_commit', $9, 'formal_commit', 'verified_hmac',
            '{"probe":"corrupt"}'
        ) RETURNING sequence
        "#,
    )
    .bind(format!("corrupt_command_{suffix}"))
    .bind(format!("corrupt_event_{suffix}"))
    .bind(format!("corrupt_correlation_{suffix}"))
    .bind(format!("corrupt_causation_{suffix}"))
    .bind(format!("corrupt_campaign_{suffix}"))
    .bind(format!("corrupt_authority_{suffix}"))
    .bind(format!("corrupt_trace_{suffix}"))
    .bind(format!("corrupt_stream_{suffix}"))
    .bind(CORRUPT_ROW_REQUEST_HASH)
    .fetch_one(&mut *corruption_transaction)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO event_outbox (
            event_id, event_sequence, nats_subject, idempotency_key,
            visibility_label, correlation_id, causation_id, payload_json,
            campaign_id, stream_id, event_schema_version,
            idempotency_operation, request_hash, request_hash_source,
            integrity_status
        ) VALUES (
            $1, $1, 'trpg.events.appended', $2, 'keeper_only', $3, $4,
            '{"probe":"corrupt"}'::jsonb, $5, $6, 1,
            'canonical_commit', $7, 'formal_commit', 'verified_hmac'
        )
        "#,
    )
    .bind(corrupt_sequence)
    .bind(format!("corrupt_outbox_{suffix}"))
    .bind(format!("corrupt_correlation_{suffix}"))
    .bind(format!("corrupt_causation_{suffix}"))
    .bind(format!("corrupt_campaign_{suffix}"))
    .bind(format!("corrupt_stream_{suffix}"))
    .bind(CORRUPT_ROW_REQUEST_HASH)
    .execute(&mut *corruption_transaction)
    .await
    .unwrap();
    corruption_transaction.commit().await.unwrap();

    let publisher = JetStreamOutboxPublisher::connect(
        &database_url,
        &nats_url,
        "p02-jetstream-publisher",
        None,
    )
    .await
    .unwrap();
    publisher.ensure_stream().await.unwrap();
    let result = publisher.publish_batch().await.unwrap();
    assert_eq!(result.claimed, 6);
    assert_eq!(result.published, 4);
    assert_eq!(result.failed, 2);
    assert_eq!(result.dead_lettered, 0);
    assert!(publisher.stream_message_count().await.unwrap() >= 4);
    assert_eq!(publisher.pending_count().await.unwrap(), 2);
    let legacy_delivery: (bool, Option<String>, String) = sqlx::query_as(
        "SELECT published_at IS NOT NULL, commit_id, integrity_status FROM event_outbox WHERE event_sequence = $1",
    )
    .bind(legacy_sequence)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        legacy_delivery,
        (true, None, "historical_unsigned".to_owned())
    );
    let corrupt_delivery: (bool, i32, Option<String>, bool) = sqlx::query_as(
        "SELECT published_at IS NULL, retry_count, last_error, claim_owner IS NULL FROM event_outbox WHERE event_sequence = $1",
    )
    .bind(corrupt_sequence)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        corrupt_delivery,
        (true, 1, Some("JETSTREAM_PUBLISH_FAILED".to_owned()), true)
    );
    let poisoned_header_delivery: (bool, i32, Option<String>, bool) = sqlx::query_as(
        "SELECT published_at IS NULL, retry_count, last_error, claim_owner IS NULL FROM event_outbox WHERE event_sequence = $1",
    )
    .bind(poisoned_header_sequence)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        poisoned_header_delivery,
        (true, 1, Some("JETSTREAM_PUBLISH_FAILED".to_owned()), true)
    );

    let cache = RedisProjectionCache::connect(&redis_url, "p02:projection:test")
        .await
        .unwrap();
    let cache_key = format!("campaign:{suffix}:clues");
    cache
        .put(&ProjectionCacheEntry {
            key: cache_key.clone(),
            version: 2,
            visibility_label: "keeper_only".to_owned(),
            visibility_subject: "not_applicable".to_owned(),
            provenance_kind: "rules_engine_decision".to_owned(),
            provenance_reference: format!("jetstream_decision_{suffix}"),
            value_json: r#"{"count":1}"#.to_owned(),
            ttl_seconds: 60,
        })
        .await
        .unwrap();
    assert_eq!(cache.get(&cache_key).await.unwrap().unwrap().version, 2);
    assert!(cache
        .put(&ProjectionCacheEntry {
            key: cache_key.clone(),
            version: 1,
            visibility_label: "public".to_owned(),
            visibility_subject: "not_applicable".to_owned(),
            provenance_kind: "system_fixture".to_owned(),
            provenance_reference: "stale_projection".to_owned(),
            value_json: r#"{"count":0}"#.to_owned(),
            ttl_seconds: 60,
        })
        .await
        .is_err());
    let retained = cache.get(&cache_key).await.unwrap().unwrap();
    assert_eq!(retained.version, 2);
    assert_eq!(retained.visibility_label, "keeper_only");
    cache.invalidate(&cache_key).await.unwrap();
}
