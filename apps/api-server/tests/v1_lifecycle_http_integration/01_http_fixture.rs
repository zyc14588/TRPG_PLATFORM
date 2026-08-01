use std::collections::HashMap;
use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use api_server::ApiApplication;
use serde_json::{json, Value};
use sqlx::postgres::PgPoolOptions;
use sqlx::Row;
use trpg_contracts::{HttpRequest, HttpResponse};
use trpg_data_eventing::event_store_sqlx_outbox_projection::PostgresCanonicalStore;
use trpg_identity::{CampaignRole, GlobalRole, IdentityService};
use trpg_ruleset_coc7::character_combat_san_chase::parse_scenario_yaml;
use trpg_security_governance::policy_adapter::{
    HttpPolicyEndpoint, OpenFgaOpaPolicyAdapter, PolicyBackend,
};
use trpg_security_governance::security_privacy::PostgresDeletionRepository;
use trpg_security_governance::tamper_evident_audit::FileAuditLog;
use trpg_shared_kernel::{
    AuthorityContract, AuthorityContractDraft, AuthorityMode, AuthorityVersionSnapshotDraft,
};

const IDENTITY_KEY: [u8; 32] = [0x31; 32];
const INTEGRITY_KEY: [u8; 32] = [0x42; 32];
const PAYLOAD_KEY: [u8; 32] = [0x53; 32];
const AUDIT_KEY: [u8; 32] = [0x64; 32];
const PASSWORD: &str = "AR06 integration password long enough";
const CAMPAIGN_ID: &str = "campaign_ar06_http";
const CHILD_CAMPAIGN_ID: &str = "campaign_ar06_http_fork";
const BOOTSTRAP_ID: &str = "bootstrap_ar06_http";
const KEEPER_ID: &str = "keeper_ar06_http";
const PLAYER_ID: &str = "player_ar06_http";
const OUTSIDER_ID: &str = "outsider_ar06_http";
const CHARACTER_ID: &str = "character_ar06_http";
const SESSION_ID: &str = "session_ar06_http";

fn required(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("{name} is required for the AR06 real-service gate"))
}

fn now_unix_ms() -> u64 {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_millis(),
    )
    .expect("current time fits u64")
}

fn authority(campaign_id: &str, created_at_unix_ms: u64) -> AuthorityContract {
    AuthorityContract::new_locked(AuthorityContractDraft {
        contract_id: format!("authority_contract_{campaign_id}_1"),
        campaign_id: campaign_id.to_owned(),
        mode: AuthorityMode::HumanKp,
        authority_owner: KEEPER_ID.to_owned(),
        version: 1,
        snapshot: AuthorityVersionSnapshotDraft {
            ruleset_version: "coc7_rules_1".to_owned(),
            house_rules_version: "house_rules_1".to_owned(),
            scenario_version: "scenario_1".to_owned(),
            prompt_version: "prompt_1".to_owned(),
            agent_pack_version: "agent_pack_1".to_owned(),
            tool_schema_version: "tool_schema_1".to_owned(),
            safety_profile_version: "safety_profile_1".to_owned(),
            ai_provider_snapshot: "provider_snapshot_1".to_owned(),
            model_route_snapshot: "model_route_1".to_owned(),
            character_sheet_template_version: "character_template_1".to_owned(),
        },
        created_at_unix_ms,
    })
    .expect("valid locked AR06 authority")
}

fn authority_body(campaign_id: &str) -> Value {
    json!({
        "contract_id": format!("authority_contract_{campaign_id}_1"),
        "authority_mode": "HUMAN_KP",
        "authority_owner": KEEPER_ID,
        "ruleset_version": "coc7_rules_1",
        "house_rules_version": "house_rules_1",
        "scenario_version": "scenario_1",
        "prompt_version": "prompt_1",
        "agent_pack_version": "agent_pack_1",
        "tool_schema_version": "tool_schema_1",
        "safety_profile_version": "safety_profile_1",
        "ai_provider_snapshot": "provider_snapshot_1",
        "model_route_snapshot": "model_route_1",
        "character_sheet_template_version": "character_template_1"
    })
}

fn command(suffix: &str, expected_version: i64) -> Value {
    json!({
        "command_id": format!("command_ar06_{suffix}"),
        "idempotency_key": format!("idempotency_ar06_{suffix}"),
        "expected_version": expected_version,
        "correlation_id": format!("correlation_ar06_{suffix}"),
        "causation_id": format!("causation_ar06_{suffix}"),
        "trace_id": format!("trace_ar06_{suffix}")
    })
}

fn character_sheet(display_name: &str) -> String {
    json!({
        "name": display_name,
        "age": 34,
        "occupation": "Archivist",
        "era": "1920s",
        "birthplace": "Brisbane",
        "characteristics": {
            "strength": 45,
            "dexterity": 55,
            "power": 65,
            "constitution": 50,
            "size": 50,
            "appearance": 60,
            "intelligence": 70,
            "education": 75,
            "luck": 55
        },
        "skills": {
            "Library Use": 75,
            "Spot Hidden": 60
        },
        "backstory_anchors": [
            "Protects fragile records",
            "Will not abandon a colleague"
        ]
    })
    .to_string()
}

fn request(method: &str, path: &str, token: Option<&str>, body: Option<Value>) -> HttpRequest {
    let mut headers = HashMap::new();
    if let Some(token) = token {
        headers.insert("authorization".to_owned(), format!("Bearer {token}"));
    }
    let body = body.map_or_else(Vec::new, |body| {
        headers.insert("content-type".to_owned(), "application/json".to_owned());
        serde_json::to_vec(&body).expect("serialize AR06 request")
    });
    HttpRequest {
        method: method.to_owned(),
        path: path.to_owned(),
        headers,
        body,
    }
}

fn call(
    application: &ApiApplication,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> HttpResponse {
    application
        .handle(&request(method, path, token, body))
        .unwrap_or_else(|| panic!("published AR06 route missing: {method} {path}"))
}

fn expect_status(response: &HttpResponse, expected: u16, operation: &str) {
    assert_eq!(
        response.status, expected,
        "unexpected status for {operation}: {}",
        response.body
    );
}

fn login(application: &ApiApplication, login: &str) -> String {
    let response = call(
        application,
        "POST",
        "/auth/login",
        None,
        Some(json!({"login": login, "password": PASSWORD})),
    );
    expect_status(&response, 200, "login");
    response.body["access_token"]
        .as_str()
        .expect("login response access token")
        .to_owned()
}

fn policy() -> OpenFgaOpaPolicyAdapter {
    let openfga_address = required("AR06_OPENFGA_ADDRESS")
        .parse::<SocketAddr>()
        .expect("valid AR06 OpenFGA address");
    let openfga_store = required("AR06_OPENFGA_STORE_ID");
    let openfga_model = required("AR06_OPENFGA_MODEL_ID");
    let opa_address = required("AR06_OPA_ADDRESS")
        .parse::<SocketAddr>()
        .expect("valid AR06 OPA address");
    let opa_revision = required("AR06_OPA_REVISION");
    OpenFgaOpaPolicyAdapter::new(
        HttpPolicyEndpoint::new(
            openfga_address,
            format!("/stores/{openfga_store}/check"),
            PolicyBackend::OpenFga,
            openfga_model,
        )
        .expect("valid OpenFGA endpoint"),
        HttpPolicyEndpoint::new(
            opa_address,
            "/v1/data/security_governance/decision",
            PolicyBackend::Opa,
            opa_revision,
        )
        .expect("valid OPA endpoint"),
    )
    .expect("valid AR06 policy pair")
}
