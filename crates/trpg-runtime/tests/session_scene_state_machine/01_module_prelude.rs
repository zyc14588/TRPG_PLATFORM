use std::env;
use std::str::FromStr;

use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{PgPool, Row};
use trpg_data_eventing::event_store_sqlx_outbox_projection::{
    PolicyAuditDraft, PostgresCanonicalStore,
};
use trpg_data_eventing::persistence_postgresql::{
    AuthorityContractSnapshot, CoreCommandMetadata, CoreDomainRepository,
    CoreDomainRepositoryError, CreateCampaignRequest, ImportScenarioRequest, StartSessionRequest,
    SwitchSceneRequest,
};
use trpg_domain_core::domain_entities_value_objects::SessionState;
use trpg_ruleset_coc7::character_combat_san_chase::parse_scenario_yaml;
use trpg_runtime::session_runtime::{
    DurableSceneState, DurableSessionState, SessionSceneStateError, SessionSceneStateMachine,
};
use trpg_shared_kernel::EventActorOriginWire;

const INTEGRITY_KEY: &[u8; 32] = &[0x58; 32];
const PAYLOAD_KEY: &[u8; 32] = &[0x69; 32];
const CAMPAIGN_ID: &str = "campaign_p06_runtime";
const AUTHORITY_ID: &str = "authority_campaign_p06_runtime_1";
const KEEPER_ID: &str = "keeper_p06_runtime";
const ROOM_ID: &str = "room_p06_runtime";
const SCENARIO_ID: &str = "scenario_p06_runtime";
const NOW_MS: u64 = 2_100_000_000_000;

async fn reset_database(url: &str, expected_database: &str, witness: bool) -> PgPool {
    assert_eq!(
        env::var("P06_ALLOW_DATABASE_RESET").as_deref(),
        Ok("1"),
        "P06 runtime test requires explicit dedicated-database reset authorization"
    );
    let options = PgConnectOptions::from_str(url).expect("valid P06 PostgreSQL URL");
    assert!(matches!(
        options.get_host(),
        "localhost" | "127.0.0.1" | "::1"
    ));
    assert_eq!(options.get_database(), Some(expected_database));
    let pool = PgPoolOptions::new()
        .max_connections(20)
        .connect_with(options)
        .await
        .expect("connect dedicated P06 database");
    let reset_sql = if witness {
        "DROP SCHEMA public CASCADE; CREATE SCHEMA public; GRANT ALL ON SCHEMA public TO public;"
    } else {
        "DROP SCHEMA IF EXISTS core_domain CASCADE; \
         DROP SCHEMA public CASCADE; \
         CREATE SCHEMA public; \
         GRANT ALL ON SCHEMA public TO public;"
    };
    sqlx::raw_sql(reset_sql)
        .execute(&pool)
        .await
        .expect("reset dedicated P06 runtime schemas");
    pool
}

#[allow(clippy::too_many_arguments)]
fn metadata(
    stream_id: &str,
    resource_type: &str,
    _action: &str,
    expected_version: i64,
    suffix: &str,
) -> CoreCommandMetadata {
    CoreCommandMetadata {
        commit_id: format!("commit_{suffix}"),
        command_id: format!("command_{suffix}"),
        idempotency_key: format!("idempotency_{suffix}"),
        expected_version,
        requesting_actor_id: KEEPER_ID.to_owned(),
        requesting_actor_role: "human_keeper".to_owned(),
        authenticated_actor_id: "workflow_p06_runtime".to_owned(),
        authenticated_actor_role: "workflow".to_owned(),
        authenticated_actor_origin: EventActorOriginWire::Workload {
            role: "workflow_engine".to_owned(),
        },
        authority_mode: "human_kp".to_owned(),
        authority_contract_version: 1,
        authority_contract_id: AUTHORITY_ID.to_owned(),
        authority_owner: KEEPER_ID.to_owned(),
        visibility_label: "party_visible".to_owned(),
        visibility_subject: "not_applicable".to_owned(),
        data_subject_id: "not_applicable".to_owned(),
        provenance_kind: "human_keeper_statement".to_owned(),
        provenance_reference: format!("keeper_decision_{suffix}"),
        provenance_recorded_by: KEEPER_ID.to_owned(),
        correlation_id: format!("correlation_{suffix}"),
        causation_id: format!("causation_{suffix}"),
        trace_id: format!("trace_{suffix}"),
        audit: PolicyAuditDraft {
            actor_id: "workflow_p06_runtime".to_owned(),
            actor_origin: "workload".to_owned(),
            authentication_reference: "workflow_p06_runtime".to_owned(),
            resource_type: resource_type.to_owned(),
            resource_id: stream_id.to_owned(),
            action: "write_official_state".to_owned(),
            requested_role: "workflow".to_owned(),
            openfga_decision_id: format!("lower_layer_fixture_openfga_{suffix}"),
            openfga_policy_revision: "lower-layer-formal-decision-fixture-v1".to_owned(),
            opa_decision_id: format!("lower_layer_fixture_opa_{suffix}"),
            opa_policy_revision: "lower-layer-formal-decision-fixture-v1".to_owned(),
        },
    }
}

fn session_request(session_id: &str, scene_id: &str, suffix: u64) -> StartSessionRequest {
    StartSessionRequest {
        session_id: session_id.to_owned(),
        campaign_id: CAMPAIGN_ID.to_owned(),
        room_id: ROOM_ID.to_owned(),
        scenario_id: SCENARIO_ID.to_owned(),
        scene_id: scene_id.to_owned(),
        scene_key: "scene_archive_front".to_owned(),
        scene_name: format!("Opening Scene {suffix}"),
        started_at_unix_ms: NOW_MS + suffix,
    }
}
