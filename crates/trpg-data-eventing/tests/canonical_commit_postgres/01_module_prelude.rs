use std::env;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use sqlx::postgres::PgConnectOptions;
use sqlx::{Connection, PgConnection, PgPool, Row};
use trpg_data_eventing::event_store_sqlx_outbox_projection::{
    AtomicCommitDraft, CanonicalEventDraft, CanonicalStoreError, PolicyAuditDraft,
    PostgresCanonicalCommitPort, PostgresCanonicalStore, RecoveryReport,
};
use trpg_data_eventing::persistence::FormalCommitRecord;
use trpg_domain_core::ddd::FactSource;
use trpg_shared_kernel::{
    CanonicalCommitKey, CanonicalCommitPort, EventActorOriginWire, TrpgError,
};

const KEY: &[u8; 32] = &[0x9c; 32];
const PAYLOAD_KEY: &[u8; 32] = &[0xad; 32];

fn database_urls() -> (String, String) {
    let primary = env::var("P02_CANONICAL_DATABASE_URL")
        .expect("P02_CANONICAL_DATABASE_URL is required for the real PostgreSQL gate");
    let witness = env::var("P02_CANONICAL_WITNESS_DATABASE_URL")
        .expect("P02_CANONICAL_WITNESS_DATABASE_URL is required for the real PostgreSQL gate");
    (primary, witness)
}

#[derive(Debug, PartialEq, Eq)]
struct DatabaseIdentity {
    host: String,
    port: u16,
    database: String,
}

fn database_identity(options: &PgConnectOptions) -> DatabaseIdentity {
    let database = options
        .get_database()
        .filter(|database| !database.trim().is_empty())
        .expect("canonical PostgreSQL URL must name an explicit non-empty database");
    DatabaseIdentity {
        host: options.get_host().to_owned(),
        port: options.get_port(),
        database: database.to_owned(),
    }
}

fn assert_distinct_database_targets(primary_url: &str, witness_url: &str) {
    let primary_identity = database_identity(
        &PgConnectOptions::from_str(primary_url).expect("valid primary canonical PostgreSQL URL"),
    );
    let witness_identity = database_identity(
        &PgConnectOptions::from_str(witness_url).expect("valid witness canonical PostgreSQL URL"),
    );
    assert_ne!(
        primary_identity, witness_identity,
        "canonical primary and witness reset targets must be distinct"
    );
}

async fn reset_dedicated_database(database_url: &str, authorized_database_variable: &str) {
    assert_eq!(
        env::var("P02_CANONICAL_ALLOW_DATABASE_RESET").as_deref(),
        Ok("1"),
        "set P02_CANONICAL_ALLOW_DATABASE_RESET=1 for the dedicated canonical integration databases"
    );
    let options = PgConnectOptions::from_str(database_url).expect("valid canonical PostgreSQL URL");
    let identity = database_identity(&options);
    let authorized_database = env::var(authorized_database_variable).unwrap_or_else(|_| {
        panic!("{authorized_database_variable} must name the dedicated canonical database")
    });
    assert!(
        !authorized_database.trim().is_empty(),
        "{authorized_database_variable} must name a non-empty dedicated canonical database"
    );
    assert!(
        matches!(identity.host.as_str(), "localhost" | "127.0.0.1" | "::1")
            && identity.database == authorized_database,
        "canonical integration test refuses to reset a non-dedicated local database"
    );
    let pool = PgPool::connect_with(options)
        .await
        .expect("connect to dedicated canonical integration database");
    sqlx::raw_sql(
        "DROP SCHEMA IF EXISTS core_domain CASCADE; \
         DROP SCHEMA public CASCADE; \
         CREATE SCHEMA public; \
         GRANT ALL ON SCHEMA public TO public;",
    )
    .execute(&pool)
    .await
    .expect("reset dedicated canonical integration database");
    pool.close().await;
}

fn draft(commit_id: &str, expected_version: i64, event_types: &[&str]) -> AtomicCommitDraft {
    AtomicCommitDraft {
        commit_id: commit_id.to_owned(),
        campaign_id: "campaign_atomic_commit".to_owned(),
        stream_id: "campaign_atomic_commit".to_owned(),
        idempotency_key: format!("idempotency_{commit_id}"),
        expected_version,
        command_id: format!("command_{commit_id}"),
        authenticated_actor_id: "workflow_atomic_commit".to_owned(),
        authenticated_actor_role: "workflow".to_owned(),
        authenticated_actor_origin: EventActorOriginWire::Workload {
            role: "workflow_engine".to_owned(),
        },
        authority_mode: "human_kp".to_owned(),
        authority_contract_version: 1,
        authority_contract_id: "authority_campaign_atomic_commit_1".to_owned(),
        authority_owner: "keeper_atomic_commit".to_owned(),
        visibility_label: "party_visible".to_owned(),
        visibility_subject: "not_applicable".to_owned(),
        data_subject_id: "not_applicable".to_owned(),
        provenance_kind: "rules_engine_decision".to_owned(),
        provenance_reference: format!("decision_{commit_id}"),
        provenance_recorded_by: "rules_engine_atomic_commit".to_owned(),
        correlation_id: format!("correlation_{commit_id}"),
        causation_id: format!("causation_{commit_id}"),
        trace_id: format!("trace_{commit_id}"),
        events: event_types
            .iter()
            .enumerate()
            .map(|(index, event_type)| CanonicalEventDraft {
                event_type: (*event_type).to_owned(),
                payload_json: format!(r#"{{"index":{index},"commit":"{commit_id}"}}"#),
                visibility: None,
                projection_targets: Vec::new(),
            })
            .collect(),
        audit: PolicyAuditDraft {
            actor_id: "keeper_atomic_commit".to_owned(),
            actor_origin: "user_session".to_owned(),
            authentication_reference: "session_atomic_commit".to_owned(),
            resource_type: "campaign".to_owned(),
            resource_id: "campaign_atomic_commit".to_owned(),
            action: "write_official_state".to_owned(),
            requested_role: "human_keeper".to_owned(),
            openfga_decision_id: format!("fga_{commit_id}"),
            openfga_policy_revision: "fga_model_atomic_commit".to_owned(),
            opa_decision_id: format!("opa_{commit_id}"),
            opa_policy_revision: "opa_bundle_atomic_commit".to_owned(),
        },
    }
}

fn bind_campaign(draft: &mut AtomicCommitDraft, campaign_id: &str) {
    draft.campaign_id = campaign_id.to_owned();
    draft.stream_id = campaign_id.to_owned();
    draft.audit.resource_id = campaign_id.to_owned();
    draft.authority_contract_id = format!("authority_{campaign_id}_1");
}

fn bind_stream(draft: &mut AtomicCommitDraft, stream_id: &str) {
    draft.stream_id = stream_id.to_owned();
    draft.audit.resource_type = "scene".to_owned();
    draft.audit.resource_id = stream_id.to_owned();
}

async fn scalar(pool: &PgPool, sql: &str) -> i64 {
    sqlx::query(sql).fetch_one(pool).await.unwrap().get(0)
}
