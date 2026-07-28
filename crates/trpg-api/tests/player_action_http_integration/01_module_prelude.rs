use std::collections::HashMap;
use std::env;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures_util::StreamExt;
use serde_json::{json, Value};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{PgPool, Row};
use trpg_api::api_contracts::{
    ApiCommandFields, AuthorizedCoreApiContext, ConfirmPlayerActionApiRequest, CoreApiError,
    PlayerActionApi, SubmitPlayerActionApiRequest,
};
use trpg_contracts::{HttpRequest, HttpResponse};
use trpg_data_eventing::event_bus_nats_impl::JetStreamOutboxPublisher;
use trpg_data_eventing::event_store_sqlx_outbox_projection::{
    PolicyAuditDraft, PostgresCanonicalStore,
};
use trpg_data_eventing::persistence_postgresql::{
    AcceptInviteRequest, AuthorityContractSnapshot, CoreCommandMetadata, CoreDomainRepository,
    CreateCampaignRequest, CreateCharacterRequest, ImportScenarioRequest, IssueInviteRequest,
    MembershipRole, StartSessionRequest,
};
use trpg_identity::{CampaignRole, GlobalRole, IdentityService, WorkloadRole};
use trpg_ruleset_coc7::character_combat_san_chase::parse_scenario_yaml;
use trpg_security_governance::formal_commit_audit::{FormalCommitAudit, FormalCommitAuthorizer};
use trpg_security_governance::policy_adapter::{
    HttpPolicyEndpoint, OpenFgaOpaPolicyAdapter, PolicyBackend,
};
use trpg_security_governance::tamper_evident_audit::FileAuditLog;
use trpg_shared_kernel::{
    AuthenticatedCommandContext, AuthorityMode, CommandEnvelope, CommandMetadata, EntityId,
    EventActorOriginWire, EventEnvelopeWire, FactProvenance, FormalWritePath, ProvenanceKind,
    ResourceRef, Visibility, VisibilityLabel,
};

use production_player_action::RepositoryPlayerActionPort;

const INTEGRITY_KEY: &[u8; 32] = &[0x47; 32];
const PAYLOAD_KEY: &[u8; 32] = &[0x58; 32];
const CAMPAIGN_ID: &str = "campaign_p07_http";
const AUTHORITY_ID: &str = "authority_contract_campaign_p07_http_1";
const KEEPER_ID: &str = "keeper_p07_http";
const OTHER_KEEPER_ID: &str = "keeper_p07_http_other";
const PLAYER_ID: &str = "player_p07_http";
const CHARACTER_ID: &str = "character_p07_http";
const ACTION_ID: &str = "action_p07_http";
const NOW_MS: u64 = 2_500_000_000_000;

fn now_unix_ms() -> u64 {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_millis(),
    )
    .expect("current time fits u64")
}

async fn reset_database(url: &str, expected: &str, witness: bool) -> PgPool {
    assert_eq!(
        env::var("P07_ALLOW_DATABASE_RESET").as_deref(),
        Ok("1"),
        "P07 HTTP test requires explicit dedicated-database reset authorization"
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
    let reset = if witness {
        "DROP SCHEMA public CASCADE; CREATE SCHEMA public; \
         GRANT ALL ON SCHEMA public TO public;"
    } else {
        "DROP SCHEMA IF EXISTS core_domain CASCADE; \
         DROP SCHEMA public CASCADE; CREATE SCHEMA public; \
         GRANT ALL ON SCHEMA public TO public;"
    };
    sqlx::raw_sql(reset)
        .execute(&pool)
        .await
        .expect("reset dedicated P07 schemas");
    pool
}

#[allow(clippy::too_many_arguments)]
fn seed_metadata(
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
        authenticated_actor_id: "workflow_p07_http_seed".to_owned(),
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
            actor_id: "workflow_p07_http_seed".to_owned(),
            actor_origin: "workload".to_owned(),
            authentication_reference: "workflow_p07_http_seed".to_owned(),
            resource_type: resource_type.to_owned(),
            resource_id: stream_id.to_owned(),
            action: "write_official_state".to_owned(),
            requested_role: "workflow".to_owned(),
            openfga_decision_id: format!("openfga_{suffix}"),
            openfga_policy_revision: "p07-http-seed-v1".to_owned(),
            opa_decision_id: format!("opa_{suffix}"),
            opa_policy_revision: "p07-http-seed-v1".to_owned(),
        },
    }
}

fn authority_snapshot() -> AuthorityContractSnapshot {
    AuthorityContractSnapshot {
        contract_id: AUTHORITY_ID.to_owned(),
        authority_mode: "HUMAN_KP".to_owned(),
        authority_owner: KEEPER_ID.to_owned(),
        ruleset_version: "coc7_rules_1".to_owned(),
        house_rules_version: "house_rules_1".to_owned(),
        scenario_version: "tutorial_mist_archive_0_1_0".to_owned(),
        prompt_version: "p07_1".to_owned(),
        agent_pack_version: "none_1".to_owned(),
        tool_schema_version: "tools_1".to_owned(),
        safety_profile_version: "safety_1".to_owned(),
        ai_provider_snapshot: "not_applicable".to_owned(),
        model_route_snapshot: "not_applicable".to_owned(),
        character_sheet_template_version: "coc7_sheet_1".to_owned(),
    }
}

fn character_sheet() -> String {
    json!({
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

fn policy() -> OpenFgaOpaPolicyAdapter {
    let openfga_address = env::var("P02_OPENFGA_ADDRESS")
        .expect("P02_OPENFGA_ADDRESS required")
        .parse()
        .expect("valid OpenFGA address");
    let openfga_store = env::var("P02_OPENFGA_STORE_ID").expect("P02_OPENFGA_STORE_ID required");
    let openfga_model = env::var("P02_OPENFGA_MODEL_ID").expect("P02_OPENFGA_MODEL_ID required");
    let opa_address = env::var("P02_OPA_ADDRESS")
        .expect("P02_OPA_ADDRESS required")
        .parse()
        .expect("valid OPA address");
    let opa_revision = env::var("P02_OPA_REVISION").expect("P02_OPA_REVISION required");
    OpenFgaOpaPolicyAdapter::new(
        HttpPolicyEndpoint::new(
            openfga_address,
            format!("/stores/{openfga_store}/check"),
            PolicyBackend::OpenFga,
            openfga_model,
        )
        .unwrap(),
        HttpPolicyEndpoint::new(
            opa_address,
            "/v1/data/security_governance/decision",
            PolicyBackend::Opa,
            opa_revision,
        )
        .unwrap(),
    )
    .unwrap()
}

struct SessionTokens {
    player: String,
    keeper: String,
    other_keeper: String,
}

fn identity(now: u64) -> (IdentityService, SessionTokens) {
    const PASSWORD: &str = "p07 http fixture password long enough";
    let contract = trpg_test_support::authority_contract_with_owner(
        CAMPAIGN_ID,
        AuthorityMode::HumanKp,
        KEEPER_ID,
        1,
    )
    .unwrap();
    assert_eq!(contract.contract_id().as_str(), AUTHORITY_ID);
    let mut identity = trpg_test_support::identity_service_for_contract(&contract);
    identity
        .create_user(
            PLAYER_ID,
            "player-p07-http@example.test",
            PASSWORD,
            GlobalRole::User,
        )
        .unwrap();
    identity
        .create_user(
            OTHER_KEEPER_ID,
            "keeper-p07-http-other@example.test",
            PASSWORD,
            GlobalRole::User,
        )
        .unwrap();
    let registrar_session = identity
        .login(
            "test-authority-registrar@example.test",
            "test authority password long enough",
            now,
        )
        .unwrap();
    let registrar = identity
        .authenticate_session(Some(registrar_session.token.expose()), now + 1)
        .unwrap();
    identity
        .grant_membership(
            &registrar,
            CAMPAIGN_ID,
            PLAYER_ID,
            CampaignRole::Player,
            now + 1,
        )
        .unwrap();
    identity
        .grant_membership(
            &registrar,
            CAMPAIGN_ID,
            OTHER_KEEPER_ID,
            CampaignRole::CampaignOwner,
            now + 1,
        )
        .unwrap();
    let player = identity
        .login("player-p07-http@example.test", PASSWORD, now + 2)
        .unwrap()
        .token
        .expose()
        .to_owned();
    let keeper = identity
        .login(
            "keeper_p07_http@example.test",
            "test authority password long enough",
            now + 2,
        )
        .unwrap()
        .token
        .expose()
        .to_owned();
    let other_keeper = identity
        .login("keeper-p07-http-other@example.test", PASSWORD, now + 2)
        .unwrap()
        .token
        .expose()
        .to_owned();
    (
        identity,
        SessionTokens {
            player,
            keeper,
            other_keeper,
        },
    )
}

#[derive(Clone)]
struct HttpPlayerActionApplication {
    identity: Arc<Mutex<IdentityService>>,
    authorizer: FormalCommitAuthorizer,
    runtime: Arc<Mutex<tokio::runtime::Runtime>>,
    port: RepositoryPlayerActionPort,
}
