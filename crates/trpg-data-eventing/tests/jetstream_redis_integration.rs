use std::env;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use async_nats::jetstream::stream::{Config as StreamConfig, StorageType, SubjectTransform};
use futures_util::StreamExt;
use sqlx::migrate::Migrator;
use sqlx::postgres::PgConnectOptions;
use sqlx::PgPool;
use trpg_data_eventing::cache_redis_impl::{ProjectionCacheEntry, RedisProjectionCache};
use trpg_data_eventing::event_bus_nats_impl::{JetStreamOutboxError, JetStreamOutboxPublisher};
use trpg_data_eventing::event_store_sqlx_outbox_projection::{
    AtomicCommitDraft, CanonicalEventDraft, PolicyAuditDraft, PostgresCanonicalStore,
};
use trpg_data_eventing::outbox_projection_workers::{
    EventingMetrics, EVENTING_COMMAND_TOTAL_METRIC,
};
use trpg_data_eventing::persistence::CURRENT_EVENT_SCHEMA_VERSION;
use trpg_identity::{CampaignRole, GlobalRole, IdentityService};
use trpg_shared_kernel::{
    EntityId, EventActorOriginWire, EventEnvelopeWire, EVENT_ENVELOPE_WIRE_SCHEMA_VERSION,
};

const KEY: &[u8; 32] = &[0xa7; 32];
const PAYLOAD_KEY: &[u8; 32] = &[0x6d; 32];

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
        authenticated_actor_role: "workflow".to_owned(),
        authenticated_actor_origin: EventActorOriginWire::Workload {
            role: "workflow_engine".to_owned(),
        },
        authority_mode: "human_kp".to_owned(),
        authority_contract_version: 1,
        authority_contract_id: format!("jetstream_authority_{suffix}"),
        authority_owner: "keeper_jetstream".to_owned(),
        visibility_label: "keeper_only".to_owned(),
        visibility_subject: "not_applicable".to_owned(),
        data_subject_id: "not_applicable".to_owned(),
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

    let store = PostgresCanonicalStore::connect(
        &database_url,
        &witness_url,
        "p02-jetstream-key",
        KEY,
        "p05-jetstream-payload-key",
        PAYLOAD_KEY,
    )
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

    let metrics = Arc::new(EventingMetrics::default());
    let publisher = JetStreamOutboxPublisher::connect(
        store.clone(),
        &nats_url,
        "p02-jetstream-publisher",
        None,
    )
    .await
    .unwrap()
    .with_metrics(Arc::clone(&metrics));

    // An existing stream is not accepted merely because it has a matching
    // subject. Every configured durability/de-duplication safety field must
    // match, otherwise startup fails closed without silently rewriting it.
    let nats_client = async_nats::connect(&nats_url).await.unwrap();
    let jetstream = async_nats::jetstream::new(nats_client.clone());

    // Prove the binary was compiled with the NATS 2.10 configuration surface:
    // a server-side subject transform must survive the client round trip. The
    // unit-level comparator then verifies that this single-field drift is
    // rejected against the canonical stream contract.
    let _ = jetstream.delete_stream("P04_SUBJECT_TRANSFORM_PROBE").await;
    let mut transform_probe = jetstream
        .create_stream(StreamConfig {
            name: "P04_SUBJECT_TRANSFORM_PROBE".to_owned(),
            subjects: vec!["p04.probe.>".to_owned()],
            subject_transform: Some(SubjectTransform {
                source: "p04.probe.>".to_owned(),
                destination: "p04.transformed.>".to_owned(),
            }),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(
        transform_probe
            .info()
            .await
            .unwrap()
            .config
            .subject_transform,
        Some(SubjectTransform {
            source: "p04.probe.>".to_owned(),
            destination: "p04.transformed.>".to_owned(),
        })
    );
    jetstream
        .delete_stream("P04_SUBJECT_TRANSFORM_PROBE")
        .await
        .unwrap();

    let _ = jetstream.delete_stream("TRPG_CANONICAL_EVENTS").await;
    jetstream
        .create_stream(StreamConfig {
            name: "TRPG_CANONICAL_EVENTS".to_owned(),
            subjects: vec!["trpg.events.>".to_owned()],
            storage: StorageType::Memory,
            max_bytes: 1024,
            max_age: Duration::from_secs(60),
            duplicate_window: Duration::from_secs(1),
            deny_delete: false,
            deny_purge: false,
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(
        publisher.ensure_stream().await,
        Err(JetStreamOutboxError::Configuration(
            "jetstream_stream_contract_mismatch"
        ))
    );
    jetstream
        .delete_stream("TRPG_CANONICAL_EVENTS")
        .await
        .unwrap();

    let mut event_messages = nats_client.subscribe("trpg.events.appended").await.unwrap();
    nats_client.flush().await.unwrap();
    publisher.ensure_stream().await.unwrap();
    let result = publisher.publish_batch().await.unwrap();
    assert_eq!(result.claimed, 3);
    assert_eq!(result.published, 3);
    assert_eq!(result.failed, 0);
    assert_eq!(result.dead_lettered, 0);
    assert_eq!(result.dead_letter_total, 2);
    assert!(result.requires_operator_attention());
    assert!(publisher.stream_message_count().await.unwrap() >= 3);
    assert_eq!(
        metrics.counter_value(EVENTING_COMMAND_TOTAL_METRIC, "outbox_publish", "published"),
        3
    );
    assert_eq!(
        metrics.counter_value(EVENTING_COMMAND_TOTAL_METRIC, "outbox_publish", "failed"),
        0
    );
    let formal_metric = metrics
        .observations()
        .into_iter()
        .find(|observation| observation.correlation_id == format!("jetstream_correlation_{suffix}"))
        .expect("formal outbox metric must retain its correlation context");
    assert_eq!(
        formal_metric.causation_id,
        format!("jetstream_causation_{suffix}")
    );
    assert_eq!(formal_metric.visibility_label, "keeper_only");
    assert_eq!(formal_metric.provenance_kind, "rules_engine_decision");

    // Validate the bytes that actually crossed NATS, rather than a helper
    // serialization detached from the publisher. All authoritative fields
    // live in the versioned shared-kernel envelope.
    let mut envelopes = Vec::with_capacity(result.published);
    for _ in 0..result.published {
        let message = tokio::time::timeout(Duration::from_secs(5), event_messages.next())
            .await
            .expect("published NATS event timed out")
            .expect("NATS event subscription ended");
        envelopes.push(
            serde_json::from_slice::<EventEnvelopeWire<serde_json::Value>>(&message.payload)
                .expect("publisher must emit the canonical event envelope"),
        );
    }
    for envelope in &envelopes {
        assert!(
            envelope.schema_version == EVENT_ENVELOPE_WIRE_SCHEMA_VERSION
                && envelope.event_schema_version > 0
                && envelope.sequence > 0
                && envelope.stream_version > 0
                && !envelope.authenticated_actor_id.is_empty()
                && !envelope.authenticated_actor_role.is_empty()
                && !envelope.authority_contract_id.is_empty()
                && !envelope.authority_owner.is_empty()
                && !envelope.command_id.is_empty()
                && !envelope.resource_type.is_empty()
                && !envelope.resource_id.is_empty()
                && !envelope.trace_id.is_empty()
                && envelope.occurred_at_unix_ms > 0,
            "incomplete production envelope: {envelope:?}"
        );
    }
    assert!(envelopes
        .iter()
        .all(|envelope| envelope.sequence != u64::try_from(legacy_sequence).unwrap()));
    let formal = envelopes
        .iter()
        .find(|envelope| envelope.campaign_id == format!("jetstream_campaign_{suffix}"))
        .expect("formal event was not published");
    assert_eq!(formal.authenticated_actor_id, "workflow_jetstream");
    assert_eq!(
        formal.event_schema_version,
        u32::try_from(CURRENT_EVENT_SCHEMA_VERSION).unwrap()
    );
    assert_eq!(formal.authenticated_actor_role, "workflow");
    assert!(matches!(
        formal.authenticated_actor_origin,
        EventActorOriginWire::Workload { ref role }
            if role == "workflow_engine"
    ));
    assert_eq!(formal.resource_campaign_id, formal.campaign_id);
    assert_eq!(formal.resource_type, "campaign");
    assert_eq!(formal.resource_id, formal.campaign_id);
    assert_eq!(formal.visibility_subject, None);
    assert_eq!(formal.request_hash_source, "formal_commit");
    assert_eq!(formal.integrity_status, "verified_hmac");
    assert!(formal.integrity_hash.is_some());
    assert!(formal.payload.get("protected_payload").is_some());
    let formal_wire = serde_json::to_string(formal).unwrap();
    assert!(!formal_wire.contains("harbor ledger"));
    assert_eq!(publisher.pending_count().await.unwrap(), 0);
    let legacy_delivery: (bool, bool, Option<String>, String) = sqlx::query_as(
        "SELECT published_at IS NULL, dead_lettered_at IS NOT NULL, last_error, integrity_status FROM event_outbox WHERE event_sequence = $1",
    )
    .bind(legacy_sequence)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        legacy_delivery,
        (
            true,
            true,
            Some("UNVERIFIED_HISTORY_QUARANTINED".to_owned()),
            "historical_unsigned".to_owned()
        )
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
        (
            true,
            0,
            Some("UNVERIFIED_HISTORY_QUARANTINED".to_owned()),
            true
        )
    );
    let persistent_alert = publisher.publish_batch().await.unwrap();
    assert_eq!(persistent_alert.dead_letter_total, 2);
    assert!(persistent_alert.requires_operator_attention());

    let cache_namespace = format!("p02:projection:test:{suffix}");
    let cache = RedisProjectionCache::connect(
        &redis_url,
        &cache_namespace,
        "redis-integration-v1",
        &[0x83; 32],
    )
    .await
    .unwrap();
    let cache_key = format!("campaign:{suffix}:clues");
    let campaign_id = format!("jetstream_campaign_{suffix}");
    let keeper_id = format!("cache_keeper_{suffix}");
    let mut identity = IdentityService::new(&[0x39; 32], 60_000).unwrap();
    identity
        .create_user(
            &keeper_id,
            &format!("cache-keeper-{suffix}@example.test"),
            "correct horse battery staple",
            GlobalRole::ServerOwner,
        )
        .unwrap();
    let session = identity
        .login(
            &format!("cache-keeper-{suffix}@example.test"),
            "correct horse battery staple",
            1_000,
        )
        .unwrap();
    let authentication = identity
        .authenticate_session(Some(session.token.expose()), 1_001)
        .unwrap();
    identity
        .grant_membership(
            &authentication,
            &campaign_id,
            &keeper_id,
            CampaignRole::HumanKeeper,
            1_002,
        )
        .unwrap();
    let replay = identity
        .verifier()
        .authorize_replay(
            &authentication,
            &EntityId::new(&campaign_id).unwrap(),
            1_003,
        )
        .unwrap();
    cache
        .put(
            &ProjectionCacheEntry::new(
                &cache_key,
                &campaign_id,
                &keeper_id,
                2,
                "keeper_only",
                "not_applicable",
                "rules_engine_decision",
                format!("jetstream_decision_{suffix}"),
                r#"{"count":1}"#,
                60,
            )
            .unwrap(),
        )
        .await
        .unwrap();

    // Redis contains only hashed keys and an AEAD envelope; neither the value
    // nor its data-subject/provenance metadata is present in plaintext.
    let redis_client = redis::Client::open(redis_url.as_str()).unwrap();
    let mut redis_connection = redis::aio::ConnectionManager::new(redis_client)
        .await
        .unwrap();
    let stored_keys: Vec<String> = redis::cmd("KEYS")
        .arg(format!("{cache_namespace}:entry:*"))
        .query_async(&mut redis_connection)
        .await
        .unwrap();
    assert_eq!(stored_keys.len(), 1);
    let stored_value: String = redis::cmd("GET")
        .arg(&stored_keys[0])
        .query_async(&mut redis_connection)
        .await
        .unwrap();
    assert!(!stored_value.contains(r#"\"count\":1"#));
    assert!(!stored_value.contains(&keeper_id));
    assert!(!stored_value.contains(&campaign_id));
    assert!(!stored_value.contains("keeper_only"));
    assert!(!stored_value.contains(&format!("jetstream_decision_{suffix}")));

    assert_eq!(
        cache
            .get_authorized(&cache_key, &replay, 1_004)
            .await
            .unwrap()
            .unwrap()
            .version(),
        2
    );
    assert!(cache
        .put(
            &ProjectionCacheEntry::new(
                &cache_key,
                &campaign_id,
                &keeper_id,
                1,
                "public",
                "not_applicable",
                "system_fixture",
                "stale_projection",
                r#"{"count":0}"#,
                60,
            )
            .unwrap(),
        )
        .await
        .is_err());
    let retained = cache
        .get_authorized(&cache_key, &replay, 1_005)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retained.version(), 2);
    assert_eq!(retained.visibility_label(), "keeper_only");
    assert_eq!(cache.invalidate_subject(&keeper_id).await.unwrap(), 1);
    assert!(cache
        .get_authorized(&cache_key, &replay, 1_006)
        .await
        .unwrap()
        .is_none());
    cache.invalidate(&cache_key).await.unwrap();

    // A storage restore or privileged tamper after startup invalidates the
    // entire canonical custody. Refuse the next batch before claiming any row
    // instead of treating corruption as one recoverable message failure.
    let mut corruption_transaction = pool.begin().await.unwrap();
    sqlx::query("SET LOCAL session_replication_role = replica")
        .execute(&mut *corruption_transaction)
        .await
        .unwrap();
    let tampered_rows = sqlx::query(
        "UPDATE event_store \
            SET correlation_id = correlation_id || '_tampered' \
          WHERE campaign_id = $1",
    )
    .bind(format!("jetstream_campaign_{suffix}"))
    .execute(&mut *corruption_transaction)
    .await
    .unwrap()
    .rows_affected();
    assert_eq!(tampered_rows, 1);
    corruption_transaction.commit().await.unwrap();

    assert!(
        publisher.publish_batch().await.is_err(),
        "publisher accepted a canonical store whose keyed event chain was corrupted"
    );
    assert!(
        JetStreamOutboxPublisher::connect(
            store,
            &nats_url,
            "p02-jetstream-corruption-restart",
            None,
        )
        .await
        .is_err(),
        "publisher restart accepted corrupted canonical custody"
    );
}
