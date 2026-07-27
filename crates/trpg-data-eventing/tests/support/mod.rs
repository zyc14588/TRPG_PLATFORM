use std::env;
use std::str::FromStr;

use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::PgPool;
use trpg_data_eventing::event_store_sqlx_outbox_projection::{
    AtomicCommitDraft, CanonicalEventDraft, PolicyAuditDraft, PostgresCanonicalStore,
};
use trpg_shared_kernel::EventActorOriginWire;

pub const INTEGRITY_KEY: &[u8; 32] = &[0x74; 32];
pub const PAYLOAD_KEY: &[u8; 32] = &[0x85; 32];

pub struct P04PostgresHarness {
    pub primary: PgPool,
    pub store: PostgresCanonicalStore,
}

impl P04PostgresHarness {
    pub async fn reset() -> Self {
        assert_eq!(
            env::var("P04_ALLOW_DATABASE_RESET").as_deref(),
            Ok("1"),
            "set P04_ALLOW_DATABASE_RESET=1 for the dedicated P04 databases"
        );
        let primary_url = env::var("P04_DATABASE_URL")
            .expect("P04_DATABASE_URL must name the dedicated p04_eventing database");
        let witness_url = env::var("P04_WITNESS_DATABASE_URL").expect(
            "P04_WITNESS_DATABASE_URL must name the dedicated p04_eventing_witness database",
        );
        assert_database_name(&primary_url, "p04_eventing");
        assert_database_name(&witness_url, "p04_eventing_witness");

        let primary = connect_pool(&primary_url, 20).await;
        let witness = connect_pool(&witness_url, 10).await;
        reset_schema(&primary).await;
        reset_schema(&witness).await;

        let store = PostgresCanonicalStore::connect(
            &primary_url,
            &witness_url,
            "p04-eventing-test-key",
            INTEGRITY_KEY,
            "p05-eventing-payload-key",
            PAYLOAD_KEY,
        )
        .await
        .expect("connect canonical Event Store and independent witness");
        store
            .prepare_for_service()
            .await
            .expect("apply forward-only P04 migrations and reconcile witness");

        Self { primary, store }
    }
}

pub fn draft(
    campaign_id: &str,
    stream_id: &str,
    commit_id: &str,
    expected_version: i64,
    event_types: &[&str],
) -> AtomicCommitDraft {
    AtomicCommitDraft {
        commit_id: commit_id.to_owned(),
        campaign_id: campaign_id.to_owned(),
        stream_id: stream_id.to_owned(),
        idempotency_key: format!("idempotency_{commit_id}"),
        expected_version,
        command_id: format!("command_{commit_id}"),
        authenticated_actor_id: "workflow_p04_eventing".to_owned(),
        authenticated_actor_role: "workflow".to_owned(),
        authenticated_actor_origin: EventActorOriginWire::Workload {
            role: "workflow_engine".to_owned(),
        },
        authority_mode: "human_kp".to_owned(),
        authority_contract_version: 1,
        authority_contract_id: format!("authority_{campaign_id}_1"),
        authority_owner: "keeper_p04_eventing".to_owned(),
        visibility_label: "party_visible".to_owned(),
        visibility_subject: "not_applicable".to_owned(),
        data_subject_id: "not_applicable".to_owned(),
        provenance_kind: "rules_engine_decision".to_owned(),
        provenance_reference: format!("decision_{commit_id}"),
        provenance_recorded_by: "rules_engine_p04_eventing".to_owned(),
        correlation_id: format!("correlation_{commit_id}"),
        causation_id: format!("causation_{commit_id}"),
        trace_id: format!("trace_{commit_id}"),
        events: event_types
            .iter()
            .enumerate()
            .map(|(index, event_type)| CanonicalEventDraft {
                event_type: (*event_type).to_owned(),
                payload_json: format!(r#"{{"commit":"{commit_id}","event_index":{index}}}"#),
                visibility: None,
                projection_targets: Vec::new(),
            })
            .collect(),
        audit: PolicyAuditDraft {
            actor_id: "keeper_p04_eventing".to_owned(),
            actor_origin: "user_session".to_owned(),
            authentication_reference: "session_p04_eventing".to_owned(),
            resource_type: "scene".to_owned(),
            resource_id: stream_id.to_owned(),
            action: "write_official_state".to_owned(),
            requested_role: "human_keeper".to_owned(),
            openfga_decision_id: format!("fga_{commit_id}"),
            openfga_policy_revision: "fga_model_p04_eventing".to_owned(),
            opa_decision_id: format!("opa_{commit_id}"),
            opa_policy_revision: "opa_bundle_p04_eventing".to_owned(),
        },
    }
}

pub fn assert_database_name(database_url: &str, expected: &str) {
    let options = PgConnectOptions::from_str(database_url).expect("valid PostgreSQL URL");
    assert_eq!(
        options.get_database().unwrap_or_default(),
        expected,
        "P04 integration tests refuse to reset a non-dedicated database"
    );
}

pub async fn connect_pool(database_url: &str, maximum_connections: u32) -> PgPool {
    let options = PgConnectOptions::from_str(database_url).expect("valid PostgreSQL URL");
    PgPoolOptions::new()
        .max_connections(maximum_connections)
        .connect_with(options)
        .await
        .expect("connect dedicated P04 PostgreSQL database")
}

pub async fn reset_schema(pool: &PgPool) {
    sqlx::raw_sql(
        "DROP SCHEMA IF EXISTS core_domain CASCADE; \
         DROP SCHEMA public CASCADE; \
         CREATE SCHEMA public; \
         GRANT ALL ON SCHEMA public TO public;",
    )
    .execute(pool)
    .await
    .expect("reset dedicated P04 PostgreSQL schema");
}
