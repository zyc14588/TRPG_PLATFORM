use std::env;

use trpg_data_eventing::cache_redis_impl::{ProjectionCacheEntry, RedisProjectionCache};
use trpg_data_eventing::event_bus_nats_impl::JetStreamOutboxPublisher;
use trpg_data_eventing::event_store_sqlx_outbox_projection::{
    AtomicCommitDraft, CanonicalEventDraft, PolicyAuditDraft, PostgresCanonicalStore,
};

const KEY: &[u8; 32] = &[0xa7; 32];

fn draft(suffix: u32) -> AtomicCommitDraft {
    let commit_id = format!("jetstream_commit_{suffix}");
    AtomicCommitDraft {
        commit_id: commit_id.clone(),
        campaign_id: format!("jetstream_campaign_{suffix}"),
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
    let (Ok(database_url), Ok(witness_url), Ok(nats_url), Ok(redis_url)) = (
        env::var("P02_EVENTING_DATABASE_URL"),
        env::var("P02_EVENTING_WITNESS_DATABASE_URL"),
        env::var("P02_NATS_URL"),
        env::var("P02_REDIS_URL"),
    ) else {
        eprintln!("skipped: set P02 database, witness, NATS, and Redis URLs for this gate");
        return;
    };
    let suffix = std::process::id();
    let store =
        PostgresCanonicalStore::connect(&database_url, &witness_url, "p02-jetstream-key", KEY)
            .await
            .unwrap();
    store.prepare_for_service().await.unwrap();
    store.commit(&draft(suffix)).await.unwrap();

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
    assert_eq!(result.published, 1);
    assert_eq!(result.failed, 0);
    assert!(publisher.stream_message_count().await.unwrap() >= 1);
    assert_eq!(publisher.pending_count().await.unwrap(), 0);

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
