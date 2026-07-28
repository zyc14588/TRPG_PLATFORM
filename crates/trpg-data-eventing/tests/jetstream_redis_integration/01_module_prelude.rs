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
        "DROP SCHEMA IF EXISTS core_domain CASCADE; \
         DROP SCHEMA public CASCADE; \
         CREATE SCHEMA public; \
         GRANT ALL ON SCHEMA public TO public; \
         CREATE EXTENSION IF NOT EXISTS vector;"
    } else {
        "DROP SCHEMA IF EXISTS core_domain CASCADE; \
         DROP SCHEMA public CASCADE; \
         CREATE SCHEMA public; \
         GRANT ALL ON SCHEMA public TO public;"
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
            visibility: None,
            projection_targets: Vec::new(),
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
