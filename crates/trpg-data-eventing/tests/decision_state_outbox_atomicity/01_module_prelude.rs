use std::env;
use std::str::FromStr;

use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{PgPool, Row};
use trpg_data_eventing::event_store_sqlx_outbox_projection::{
    PolicyAuditDraft, PostgresCanonicalStore,
};
use trpg_data_eventing::persistence_postgresql::{
    AcceptInviteRequest, AuthorityContractSnapshot, CoreCommandMetadata, CoreDomainRepository,
    CoreDomainRepositoryError, CreateCampaignRequest, CreateCharacterRequest,
    ImportScenarioRequest, InvestigationExecutionRecord, IssueInviteRequest, MembershipRole,
    PlayerActionDiceRecord, PlayerActionIntentRecord, SanityExecutionRecord, StartSessionRequest,
    SubmitPlayerActionRequest,
};
use trpg_ruleset_coc7::character_combat_san_chase::parse_scenario_yaml;
use trpg_shared_kernel::EventActorOriginWire;

const INTEGRITY_KEY: &[u8; 32] = &[0x27; 32];
const PAYLOAD_KEY: &[u8; 32] = &[0x38; 32];
const CAMPAIGN_ID: &str = "campaign_p07_atomic";
const AUTHORITY_ID: &str = "authority_campaign_p07_atomic_1";
const KEEPER_ID: &str = "keeper_p07_atomic";
const PLAYER_ID: &str = "player_p07_atomic";
const CHARACTER_ID: &str = "character_p07_atomic";
const ACTION_ID: &str = "action_p07_atomic";
const NOW_MS: u64 = 2_400_000_000_000;

async fn reset_database(url: &str, expected: &str, witness: bool) -> PgPool {
    assert_eq!(
        env::var("P07_ALLOW_DATABASE_RESET").as_deref(),
        Ok("1"),
        "P07 tests require explicit dedicated-database reset authorization"
    );
    let options = PgConnectOptions::from_str(url).expect("valid P07 PostgreSQL URL");
    assert!(matches!(
        options.get_host(),
        "localhost" | "127.0.0.1" | "::1"
    ));
    assert_eq!(options.get_database(), Some(expected));
    let pool = PgPoolOptions::new()
        .max_connections(20)
        .connect_with(options)
        .await
        .expect("connect dedicated P07 PostgreSQL");
    let sql = if witness {
        "DROP SCHEMA public CASCADE; CREATE SCHEMA public; \
         GRANT ALL ON SCHEMA public TO public;"
    } else {
        "DROP SCHEMA IF EXISTS core_domain CASCADE; \
         DROP SCHEMA public CASCADE; CREATE SCHEMA public; \
         GRANT ALL ON SCHEMA public TO public;"
    };
    sqlx::raw_sql(sql).execute(&pool).await.unwrap();
    pool
}

#[allow(clippy::too_many_arguments)]
fn metadata(
    actor_id: &str,
    actor_role: &str,
    stream_id: &str,
    resource_type: &str,
    expected_version: i64,
    suffix: &str,
    visibility_label: &str,
    visibility_subject: &str,
    provenance_kind: &str,
) -> CoreCommandMetadata {
    CoreCommandMetadata {
        commit_id: format!("commit_{suffix}"),
        command_id: format!("command_{suffix}"),
        idempotency_key: format!("idempotency_{suffix}"),
        expected_version,
        requesting_actor_id: actor_id.to_owned(),
        requesting_actor_role: actor_role.to_owned(),
        authenticated_actor_id: "workflow_p07_atomic".to_owned(),
        authenticated_actor_role: "workflow".to_owned(),
        authenticated_actor_origin: EventActorOriginWire::Workload {
            role: "workflow_engine".to_owned(),
        },
        authority_mode: "human_kp".to_owned(),
        authority_contract_version: 1,
        authority_contract_id: AUTHORITY_ID.to_owned(),
        authority_owner: KEEPER_ID.to_owned(),
        visibility_label: visibility_label.to_owned(),
        visibility_subject: visibility_subject.to_owned(),
        data_subject_id: if visibility_subject == "not_applicable" {
            "not_applicable".to_owned()
        } else {
            visibility_subject.to_owned()
        },
        provenance_kind: provenance_kind.to_owned(),
        provenance_reference: format!("source_{suffix}"),
        provenance_recorded_by: actor_id.to_owned(),
        correlation_id: format!("correlation_{suffix}"),
        causation_id: format!("causation_{suffix}"),
        trace_id: format!("trace_{suffix}"),
        audit: PolicyAuditDraft {
            actor_id: "workflow_p07_atomic".to_owned(),
            actor_origin: "workload".to_owned(),
            authentication_reference: "workflow_p07_atomic".to_owned(),
            resource_type: resource_type.to_owned(),
            resource_id: stream_id.to_owned(),
            action: "write_official_state".to_owned(),
            requested_role: "workflow".to_owned(),
            openfga_decision_id: format!("openfga_{suffix}"),
            openfga_policy_revision: "p07-lower-layer-fixture-v1".to_owned(),
            opa_decision_id: format!("opa_{suffix}"),
            opa_policy_revision: "p07-lower-layer-fixture-v1".to_owned(),
        },
    }
}

fn authority() -> AuthorityContractSnapshot {
    AuthorityContractSnapshot {
        contract_id: AUTHORITY_ID.to_owned(),
        authority_mode: "HUMAN_KP".to_owned(),
        authority_owner: KEEPER_ID.to_owned(),
        ruleset_version: "coc7-1".to_owned(),
        house_rules_version: "none-1".to_owned(),
        scenario_version: "tutorial-0.1.0".to_owned(),
        prompt_version: "p07-1".to_owned(),
        agent_pack_version: "none-1".to_owned(),
        tool_schema_version: "tools-1".to_owned(),
        safety_profile_version: "safety-1".to_owned(),
        ai_provider_snapshot: "not_applicable".to_owned(),
        model_route_snapshot: "not_applicable".to_owned(),
        character_sheet_template_version: "coc7-sheet-1".to_owned(),
    }
}

fn character_sheet() -> String {
    serde_json::json!({
        "name": "Evelyn Hart",
        "age": 31,
        "occupation": "Investigative journalist",
        "era": "1920s",
        "birthplace": "Brisbane",
        "characteristics": {
            "strength": 50,
            "dexterity": 60,
            "power": 65,
            "constitution": 55,
            "size": 50,
            "appearance": 55,
            "intelligence": 70,
            "education": 75,
            "luck": 60
        },
        "skills": {
            "Library Use": 70,
            "Psychology": 55
        },
        "backstory_anchors": [
            "Protects confidential sources",
            "Distrusts official explanations"
        ]
    })
    .to_string()
}
