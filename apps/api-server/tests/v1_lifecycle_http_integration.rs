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

#[test]
fn published_v1_api_completes_the_core_lifecycle_with_real_least_privilege_logins() {
    let admin_database_url = required("AR06_ADMIN_DATABASE_URL");
    let admin_witness_database_url = required("AR06_ADMIN_WITNESS_DATABASE_URL");
    let api_database_url = required("AR06_API_DATABASE_URL");
    let canonical_database_url = required("AR06_CANONICAL_DATABASE_URL");
    let witness_database_url = required("AR06_WITNESS_DATABASE_URL");
    let redis_url = required("AR06_REDIS_URL");
    let setup_runtime = tokio::runtime::Runtime::new().expect("AR06 setup runtime");

    let migration_store = setup_runtime
        .block_on(PostgresCanonicalStore::connect(
            &admin_database_url,
            &admin_witness_database_url,
            "ar06-migration-integrity",
            &INTEGRITY_KEY,
            "ar06-migration-payload",
            &PAYLOAD_KEY,
        ))
        .expect("connect AR06 migration stores");
    setup_runtime
        .block_on(migration_store.prepare_for_service())
        .expect("apply complete migration chain");
    let witness_owner_pool = setup_runtime
        .block_on(
            PgPoolOptions::new()
                .max_connections(1)
                .connect(&admin_witness_database_url),
        )
        .expect("connect witness owner for runtime-role bootstrap");
    setup_runtime
        .block_on(
            sqlx::query(
                "ALTER ROLE trpg_witness_append_login \
                 LOGIN INHERIT NOSUPERUSER NOCREATEDB NOCREATEROLE \
                 NOREPLICATION NOBYPASSRLS PASSWORD 'ar06_witness_password'",
            )
            .execute(&witness_owner_pool),
        )
        .expect("activate the production witness append login");
    setup_runtime
        .block_on(
            sqlx::query(
                "GRANT trpg_witness_append_service \
                 TO trpg_witness_append_login",
            )
            .execute(&witness_owner_pool),
        )
        .expect("grant the production witness append role");

    let api_pool = setup_runtime
        .block_on(
            PgPoolOptions::new()
                .max_connections(4)
                .connect(&api_database_url),
        )
        .expect("connect the real trpg_api_login");
    let canonical_pool = setup_runtime
        .block_on(
            PgPoolOptions::new()
                .max_connections(2)
                .connect(&canonical_database_url),
        )
        .expect("connect the real trpg_canonical_login");
    let canonical_witness_pool = setup_runtime
        .block_on(
            PgPoolOptions::new()
                .max_connections(2)
                .connect(&witness_database_url),
        )
        .expect("connect the real trpg_witness_append_login");
    assert_eq!(
        setup_runtime
            .block_on(sqlx::query_scalar::<_, String>("SELECT current_user").fetch_one(&api_pool))
            .expect("read API login"),
        "trpg_api_login"
    );
    assert_eq!(
        setup_runtime
            .block_on(
                sqlx::query_scalar::<_, String>("SELECT current_user").fetch_one(&canonical_pool)
            )
            .expect("read canonical login"),
        "trpg_canonical_login"
    );
    assert_eq!(
        setup_runtime
            .block_on(
                sqlx::query_scalar::<_, String>("SELECT current_user")
                    .fetch_one(&canonical_witness_pool)
            )
            .expect("read witness append login"),
        "trpg_witness_append_login"
    );
    let least_privilege: (bool, bool, bool, bool, bool) = setup_runtime
        .block_on(
            sqlx::query_as(
                r#"
            SELECT has_table_privilege(current_user, 'public.campaign_exports', 'SELECT'),
                   has_table_privilege(current_user, 'public.campaign_exports', 'INSERT'),
                   has_table_privilege(current_user, 'public.campaign_exports', 'UPDATE'),
                   has_table_privilege(current_user, 'public.campaign_exports', 'DELETE'),
                   has_table_privilege(
                       current_user, 'core_domain.session_characters', 'INSERT'
                   )
            "#,
            )
            .fetch_one(&api_pool),
        )
        .expect("read AR06 API privileges");
    assert_eq!(least_privilege, (true, true, false, false, true));
    let can_read_player_action: bool = setup_runtime
        .block_on(
            sqlx::query_scalar(
                "SELECT has_table_privilege(\
                 current_user, 'public.player_actions', 'SELECT'\
             )",
            )
            .fetch_one(&api_pool),
        )
        .expect("read Player Action query privilege");
    assert!(can_read_player_action);
    let canonical_crosses_projection_boundary: bool = setup_runtime
        .block_on(
            sqlx::query_scalar(
                "SELECT has_table_privilege(\
                current_user, 'public.campaign_exports', 'INSERT'\
             ) OR has_table_privilege(\
                current_user, 'core_domain.session_characters', 'INSERT'\
             )",
            )
            .fetch_one(&canonical_pool),
        )
        .expect("read canonical projection boundary");
    assert!(!canonical_crosses_projection_boundary);
    let canonical_payload_key_privileges: (bool, bool, bool, bool) = setup_runtime
        .block_on(
            sqlx::query_as(
                r#"
            SELECT has_table_privilege(
                       current_user, 'public.privacy_subject_keys', 'SELECT'
                   ),
                   has_table_privilege(
                       current_user, 'public.privacy_subject_keys', 'INSERT'
                   ),
                   has_table_privilege(
                       current_user, 'public.privacy_subject_keys', 'UPDATE'
                   ),
                   has_table_privilege(
                       current_user, 'public.privacy_subject_keys', 'DELETE'
                   )
            "#,
            )
            .fetch_one(&canonical_pool),
        )
        .expect("read canonical subject-payload-key boundary");
    assert_eq!(canonical_payload_key_privileges, (true, true, false, false));

    let now = now_unix_ms();
    let mut identity = IdentityService::from_prepared_postgres_with_security_and_redis_tls(
        &api_database_url,
        None,
        &redis_url,
        "ar06:identity",
        &IDENTITY_KEY,
        3_600_000,
        2,
        None,
        None,
        None,
    )
    .expect("connect persistent AR06 identity through trpg_api_login");
    for (user_id, login, role) in [
        (
            BOOTSTRAP_ID,
            "bootstrap-ar06@example.test",
            GlobalRole::ServerOwner,
        ),
        (KEEPER_ID, "keeper-ar06@example.test", GlobalRole::User),
        (PLAYER_ID, "player-ar06@example.test", GlobalRole::User),
        (OUTSIDER_ID, "outsider-ar06@example.test", GlobalRole::User),
    ] {
        identity
            .create_user(user_id, login, PASSWORD, role)
            .expect("create AR06 identity");
    }
    let bootstrap_session = identity
        .login("bootstrap-ar06@example.test", PASSWORD, now)
        .expect("login AR06 authority bootstrap owner");
    let bootstrap_authentication = identity
        .authenticate_session(Some(bootstrap_session.token.expose()), now + 1)
        .expect("authenticate AR06 authority bootstrap owner");
    for (campaign_id, created_at) in [(CAMPAIGN_ID, 1_u64), (CHILD_CAMPAIGN_ID, 2_u64)] {
        identity
            .grant_membership(
                &bootstrap_authentication,
                campaign_id,
                KEEPER_ID,
                CampaignRole::HumanKeeper,
                now + 1,
            )
            .expect("pre-provision canonical keeper membership");
        identity
            .register_authority_contract(
                &bootstrap_authentication,
                authority(campaign_id, created_at),
                now + 1,
            )
            .expect("pre-provision immutable Authority Contract");
    }

    let canonical_store = setup_runtime
        .block_on(PostgresCanonicalStore::connect(
            &canonical_database_url,
            &witness_database_url,
            "ar06-service-integrity",
            &INTEGRITY_KEY,
            "ar06-service-payload",
            &PAYLOAD_KEY,
        ))
        .expect("connect canonical stores with service login");
    let privacy_runtime = tokio::runtime::Runtime::new().expect("AR06 privacy runtime");
    let deletion_repository = privacy_runtime
        .block_on(PostgresDeletionRepository::connect(&api_database_url))
        .expect("connect deletion repository through API login");
    let canonical_runtime = tokio::runtime::Runtime::new().expect("AR06 canonical runtime");
    let audit_path = PathBuf::from(format!(
        "/tmp/trpg-ar06-v1-audit-{}.jsonl",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&audit_path);
    let _ = std::fs::remove_file(audit_path.with_extension("jsonl.head"));
    let audit =
        FileAuditLog::open(&audit_path, "ar06-audit-key", &AUDIT_KEY).expect("open AR06 audit log");
    let application = ApiApplication::new_production_governed_with_v1_lifecycle(
        identity,
        policy(),
        audit,
        canonical_runtime,
        canonical_store,
        privacy_runtime,
        deletion_repository,
        &api_database_url,
    )
    .expect("compose the published V1 application without importing a Repository");

    let keeper_token = login(&application, "keeper-ar06@example.test");
    let player_token = login(&application, "player-ar06@example.test");
    let outsider_token = login(&application, "outsider-ar06@example.test");

    let unauthenticated = call(
        &application,
        "POST",
        "/api/v1/campaigns",
        None,
        Some(json!({})),
    );
    expect_status(&unauthenticated, 401, "unauthenticated Campaign create");

    let parent_campaign = json!({
        "command": command("campaign_create", 0),
        "campaign_id": CAMPAIGN_ID,
        "owner_user_id": KEEPER_ID,
        "title": "AR06 public lifecycle",
        "room_id": "room_ar06_http",
        "room_name": "AR06 table",
        "created_at_unix_ms": 1,
        "authority": authority_body(CAMPAIGN_ID)
    });
    let mut wrong_authority = parent_campaign.clone();
    wrong_authority["authority"]["authority_mode"] = json!("AI_KP");
    let wrong_authority_response = call(
        &application,
        "POST",
        "/api/v1/campaigns",
        Some(&keeper_token),
        Some(wrong_authority),
    );
    expect_status(&wrong_authority_response, 400, "wrong Authority mode");
    let event_count_before_create: i64 = setup_runtime
        .block_on(
            sqlx::query_scalar("SELECT count(*) FROM public.event_store").fetch_one(&api_pool),
        )
        .expect("count events before Campaign create");
    assert_eq!(event_count_before_create, 0);

    let outsider_create = call(
        &application,
        "POST",
        "/api/v1/campaigns",
        Some(&outsider_token),
        Some(parent_campaign.clone()),
    );
    expect_status(&outsider_create, 403, "unauthorized Campaign create");
    let created = call(
        &application,
        "POST",
        "/api/v1/campaigns",
        Some(&keeper_token),
        Some(parent_campaign.clone()),
    );
    expect_status(&created, 201, "Campaign create");
    assert_eq!(created.body["aggregate_version"], 1);
    let created_retry = call(
        &application,
        "POST",
        "/api/v1/campaigns",
        Some(&keeper_token),
        Some(parent_campaign),
    );
    expect_status(&created_retry, 201, "Campaign create retry");
    assert_eq!(
        created_retry.body["last_event_sequence"],
        created.body["last_event_sequence"]
    );

    let outsider_list = call(
        &application,
        "GET",
        "/api/v1/campaigns",
        Some(&outsider_token),
        None,
    );
    expect_status(&outsider_list, 200, "membership-filtered Campaign list");
    assert_eq!(
        outsider_list.body["campaigns"]
            .as_array()
            .expect("Campaign array")
            .len(),
        0
    );
    let invisible_campaign = call(
        &application,
        "GET",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}"),
        Some(&outsider_token),
        None,
    );
    expect_status(&invisible_campaign, 404, "opaque invisible Campaign");
    assert_eq!(invisible_campaign.body["error"], "CORE_API_NOT_FOUND");

    let invite_body = json!({
        "command": command("invite_issue", 0),
        "campaign_id": CAMPAIGN_ID,
        "invite_id": "invite_ar06_http",
        "invited_user_id": PLAYER_ID,
        "role": "PLAYER",
        "expires_at_unix_ms": now + 600_000
    });
    let invite = call(
        &application,
        "POST",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/invites"),
        Some(&keeper_token),
        Some(invite_body.clone()),
    );
    expect_status(&invite, 201, "invite issue");
    let raw_token = invite.body["raw_token"]
        .as_str()
        .expect("invite raw token")
        .to_owned();
    let invite_retry = call(
        &application,
        "POST",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/invites"),
        Some(&keeper_token),
        Some(invite_body),
    );
    expect_status(&invite_retry, 201, "invite exact retry");
    assert!(
        invite_retry.body["raw_token"].as_str() == Some(raw_token.as_str()),
        "exact retry changed the issued invite token"
    );
    assert_eq!(
        invite_retry.body["last_event_sequence"],
        invite.body["last_event_sequence"]
    );
    let accepted = call(
        &application,
        "POST",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/invites/invite_ar06_http/accept"),
        Some(&player_token),
        Some(json!({
            "command": command("invite_accept", 1),
            "campaign_id": CAMPAIGN_ID,
            "invite_id": "invite_ar06_http",
            "accepting_user_id": PLAYER_ID,
            "raw_token": raw_token
        })),
    );
    expect_status(&accepted, 200, "invite acceptance");

    let create_character = call(
        &application,
        "POST",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/characters"),
        Some(&player_token),
        Some(json!({
            "command": command("character_create", 0),
            "campaign_id": CAMPAIGN_ID,
            "character_id": CHARACTER_ID,
            "owner_user_id": PLAYER_ID,
            "display_name": "Ada Mercer",
            "sheet_version_id": "sheet_ar06_http_v1",
            "sheet_json": character_sheet("Ada Mercer")
        })),
    );
    expect_status(&create_character, 201, "Character create");

    let update_body = json!({
        "command": command("character_update", 1),
        "campaign_id": CAMPAIGN_ID,
        "character_id": CHARACTER_ID,
        "owner_user_id": PLAYER_ID,
        "display_name": "Ada Mercer Updated",
        "sheet_version_id": "sheet_ar06_http_v2",
        "sheet_json": character_sheet("Ada Mercer Updated")
    });
    let updated = call(
        &application,
        "PUT",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/characters/{CHARACTER_ID}"),
        Some(&player_token),
        Some(update_body.clone()),
    );
    expect_status(&updated, 200, "Character update");
    assert_eq!(updated.body["aggregate_version"], 2);
    let update_retry = call(
        &application,
        "PUT",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/characters/{CHARACTER_ID}"),
        Some(&player_token),
        Some(update_body.clone()),
    );
    expect_status(&update_retry, 200, "Character update exact retry");
    assert_eq!(
        update_retry.body["last_event_sequence"],
        updated.body["last_event_sequence"]
    );
    let mut conflicting_reuse = update_body;
    conflicting_reuse["display_name"] = json!("Conflicting payload");
    let conflicting_reuse_response = call(
        &application,
        "PUT",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/characters/{CHARACTER_ID}"),
        Some(&player_token),
        Some(conflicting_reuse),
    );
    expect_status(
        &conflicting_reuse_response,
        409,
        "idempotency key payload conflict",
    );
    let stale_version = call(
        &application,
        "PUT",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/characters/{CHARACTER_ID}"),
        Some(&player_token),
        Some(json!({
            "command": command("character_stale", 1),
            "campaign_id": CAMPAIGN_ID,
            "character_id": CHARACTER_ID,
            "owner_user_id": PLAYER_ID,
            "display_name": "Stale update",
            "sheet_version_id": "sheet_ar06_http_stale",
            "sheet_json": character_sheet("Stale update")
        })),
    );
    expect_status(&stale_version, 409, "concurrent Character version conflict");

    let submitted = call(
        &application,
        "POST",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/characters/{CHARACTER_ID}/submit"),
        Some(&player_token),
        Some(json!({
            "command": command("character_submit", 2),
            "campaign_id": CAMPAIGN_ID,
            "character_id": CHARACTER_ID
        })),
    );
    expect_status(&submitted, 202, "Character submit");
    let reviewed = call(
        &application,
        "POST",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/characters/{CHARACTER_ID}/review"),
        Some(&keeper_token),
        Some(json!({
            "command": command("character_review", 3),
            "campaign_id": CAMPAIGN_ID,
            "character_id": CHARACTER_ID
        })),
    );
    expect_status(&reviewed, 200, "Character review");
    let current_sheet = setup_runtime
        .block_on(
            sqlx::query(
                "SELECT sheet.version, sheet.locked \
                   FROM public.characters AS character \
                   JOIN public.character_sheet_versions AS sheet \
                     ON sheet.character_id = character.character_id \
                    AND sheet.version = character.current_sheet_version \
                  WHERE character.character_id = $1",
            )
            .bind(CHARACTER_ID)
            .fetch_one(&api_pool),
        )
        .expect("load approved current Character sheet");
    assert_eq!(current_sheet.get::<i64, _>("version"), 2);
    assert!(current_sheet.get::<bool, _>("locked"));

    let tutorial = parse_scenario_yaml(include_str!(
        "../../../fixtures/scenarios/tutorial_mist_archive.scenario.yaml"
    ))
    .expect("parse AR06 tutorial scenario");
    let imported = call(
        &application,
        "POST",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/scenarios/import"),
        Some(&keeper_token),
        Some(json!({
            "command": command("scenario_import", 0),
            "campaign_id": CAMPAIGN_ID,
            "scenario_id": "scenario_ar06_http",
            "ruleset_id": tutorial.ruleset_id,
            "format_version": tutorial.format_version,
            "content_hash": tutorial.content_hash,
            "document_json": tutorial.canonical_json
        })),
    );
    expect_status(&imported, 201, "Scenario import");
    let started = call(
        &application,
        "POST",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/sessions"),
        Some(&keeper_token),
        Some(json!({
            "command": command("session_start", 0),
            "campaign_id": CAMPAIGN_ID,
            "session_id": SESSION_ID,
            "room_id": "room_ar06_http",
            "scenario_id": "scenario_ar06_http",
            "scene_id": "scene_ar06_archive",
            "scene_key": "scene_archive_front",
            "scene_name": "Archive front",
            "started_at_unix_ms": now + 1_000
        })),
    );
    expect_status(&started, 201, "Session start");
    let joined = call(
        &application,
        "POST",
        &format!(
            "/api/v1/campaigns/{CAMPAIGN_ID}/sessions/{SESSION_ID}/characters/{CHARACTER_ID}/join"
        ),
        Some(&player_token),
        Some(json!({
            "command": command("character_join", 0),
            "join_id": "join_ar06_http",
            "campaign_id": CAMPAIGN_ID,
            "session_id": SESSION_ID,
            "character_id": CHARACTER_ID,
            "owner_user_id": PLAYER_ID,
            "joined_at_unix_ms": now + 2_000
        })),
    );
    expect_status(&joined, 201, "Character join Session");
    let switched = call(
        &application,
        "POST",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/sessions/{SESSION_ID}/scenes"),
        Some(&keeper_token),
        Some(json!({
            "command": command("scene_switch", 1),
            "campaign_id": CAMPAIGN_ID,
            "session_id": SESSION_ID,
            "next_scene_id": "scene_ar06_basement",
            "next_scene_key": "scene_basement",
            "next_scene_name": "Basement",
            "switched_at_unix_ms": now + 3_000
        })),
    );
    expect_status(&switched, 201, "Scene switch");

    let action_id = "action_ar06_http";
    let submitted_action = call(
        &application,
        "POST",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/player-actions"),
        Some(&player_token),
        Some(json!({
            "command": command("action_submit", 0),
            "campaign_id": CAMPAIGN_ID,
            "action_id": action_id,
            "character_id": CHARACTER_ID,
            "scene_id": "scene_ar06_basement",
            "submitted_at_unix_ms": now + 4_000,
            "intent": {
                "kind": "INVESTIGATION",
                "skill_name": "Library Use",
                "clue_id": "clue_wrong_signature",
                "clue_importance": "CORE",
                "adjustment": "NONE"
            }
        })),
    );
    expect_status(&submitted_action, 202, "Player action submit");
    assert_eq!(
        submitted_action.body["state"],
        "AWAITING_HUMAN_CONFIRMATION"
    );
    let unauthorized_confirmation = call(
        &application,
        "POST",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/player-actions/{action_id}/confirm"),
        Some(&outsider_token),
        Some(json!({
            "command": command("action_unauthorized_confirm", 1),
            "campaign_id": CAMPAIGN_ID,
            "action_id": action_id,
            "resolved_at_unix_ms": now + 5_000
        })),
    );
    expect_status(
        &unauthorized_confirmation,
        403,
        "unauthorized action confirmation",
    );
    let confirmed_action = call(
        &application,
        "POST",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/player-actions/{action_id}/confirm"),
        Some(&keeper_token),
        Some(json!({
            "command": command("action_confirm", 1),
            "campaign_id": CAMPAIGN_ID,
            "action_id": action_id,
            "resolved_at_unix_ms": now + 5_000
        })),
    );
    expect_status(&confirmed_action, 200, "Player action confirmation");
    assert_eq!(confirmed_action.body["state"], "RESOLVED");
    let action_sequence = confirmed_action.body["last_event_sequence"]
        .as_i64()
        .expect("confirmed action sequence");

    let reconsideration_id = "reconsideration_ar06_http";
    let reconsideration = call(
        &application,
        "POST",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/reconsiderations"),
        Some(&player_token),
        Some(json!({
            "command": command("reconsider_request", 0),
            "reconsideration_id": reconsideration_id,
            "campaign_id": CAMPAIGN_ID,
            "original_event_sequence": action_sequence,
            "requested_by": PLAYER_ID,
            "reason": "Review the canonical decision"
        })),
    );
    expect_status(&reconsideration, 202, "Reconsideration request");
    let reconsideration_review = call(
        &application,
        "POST",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/reconsiderations/{reconsideration_id}/review"),
        Some(&keeper_token),
        Some(json!({
            "command": command("reconsider_review", 1),
            "reconsideration_id": reconsideration_id,
            "campaign_id": CAMPAIGN_ID,
            "review_event_id": "review_event_ar06_http",
            "review_summary": "Canonical evidence reviewed"
        })),
    );
    expect_status(&reconsideration_review, 200, "Reconsideration review");
    let reconsideration_resolution = call(
        &application,
        "POST",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/reconsiderations/{reconsideration_id}/resolve"),
        Some(&keeper_token),
        Some(json!({
            "command": command("reconsider_resolve", 2),
            "reconsideration_id": reconsideration_id,
            "campaign_id": CAMPAIGN_ID,
            "resolution_event_id": "resolution_event_ar06_http",
            "outcome": "UPHELD",
            "resolution": "The original canonical decision stands",
            "corrected_event_type": null,
            "corrected_payload_json": null
        })),
    );
    expect_status(
        &reconsideration_resolution,
        200,
        "Reconsideration resolution",
    );

    let export_id = "export_ar06_http";
    let export = call(
        &application,
        "POST",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/exports"),
        Some(&keeper_token),
        Some(json!({
            "command": command("export_request", 0),
            "export_id": export_id,
            "campaign_id": CAMPAIGN_ID,
            "requested_by": KEEPER_ID,
            "audience": "CAMPAIGN_ARCHIVE",
            "requested_at_unix_ms": now + 6_000
        })),
    );
    expect_status(&export, 202, "Campaign export request");
    let export_query = call(
        &application,
        "GET",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/exports/{export_id}"),
        Some(&keeper_token),
        None,
    );
    expect_status(&export_query, 200, "Campaign export query");
    assert_eq!(export_query.body["state"], "REQUESTED");
    let hidden_export = call(
        &application,
        "GET",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/exports/{export_id}"),
        Some(&player_token),
        None,
    );
    expect_status(&hidden_export, 404, "keeper-only export opacity");

    let ended = call(
        &application,
        "PATCH",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/sessions/{SESSION_ID}"),
        Some(&keeper_token),
        Some(json!({
            "command": command("session_end", 2),
            "campaign_id": CAMPAIGN_ID,
            "session_id": SESSION_ID,
            "state": "ENDED",
            "changed_at_unix_ms": now + 7_000
        })),
    );
    expect_status(&ended, 200, "Session end");

    let child_campaign = call(
        &application,
        "POST",
        "/api/v1/campaigns",
        Some(&keeper_token),
        Some(json!({
            "command": command("child_campaign_create", 0),
            "campaign_id": CHILD_CAMPAIGN_ID,
            "owner_user_id": KEEPER_ID,
            "title": "AR06 fork child",
            "room_id": "room_ar06_http_fork",
            "room_name": "AR06 fork table",
            "created_at_unix_ms": 2,
            "authority": authority_body(CHILD_CAMPAIGN_ID)
        })),
    );
    expect_status(&child_campaign, 201, "Fork child Campaign create");
    let forked = call(
        &application,
        "POST",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/forks"),
        Some(&keeper_token),
        Some(json!({
            "command": command("campaign_fork", 0),
            "fork_id": "fork_ar06_http",
            "parent_campaign_id": CAMPAIGN_ID,
            "child_campaign_id": CHILD_CAMPAIGN_ID,
            "source_session_id": SESSION_ID,
            "reason": "Preserve a canonical branch"
        })),
    );
    expect_status(&forked, 201, "Campaign fork");

    let parent_for_player = call(
        &application,
        "GET",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}"),
        Some(&player_token),
        None,
    );
    expect_status(&parent_for_player, 200, "visible Campaign query");
    let child_for_player = call(
        &application,
        "GET",
        &format!("/api/v1/campaigns/{CHILD_CAMPAIGN_ID}"),
        Some(&player_token),
        None,
    );
    expect_status(&child_for_player, 404, "invisible fork query");

    let counts = setup_runtime
        .block_on(
            sqlx::query(
                r#"
                SELECT
                    (SELECT count(*) FROM public.event_store
                      WHERE event_type = 'CharacterUpdated'
                        AND campaign_id = $1) AS character_updates,
                    (SELECT count(*) FROM core_domain.session_characters
                      WHERE campaign_id = $1
                        AND character_id = $2) AS session_characters,
                    (SELECT count(*) FROM public.campaign_exports
                      WHERE campaign_id = $1
                        AND export_id = $3) AS exports,
                    (SELECT count(*) FROM public.campaign_forks
                      WHERE parent_campaign_id = $1
                        AND child_campaign_id = $4) AS forks
                "#,
            )
            .bind(CAMPAIGN_ID)
            .bind(CHARACTER_ID)
            .bind(export_id)
            .bind(CHILD_CAMPAIGN_ID)
            .fetch_one(&api_pool),
        )
        .expect("load AR06 lifecycle counts");
    assert_eq!(counts.get::<i64, _>("character_updates"), 1);
    assert_eq!(counts.get::<i64, _>("session_characters"), 1);
    assert_eq!(counts.get::<i64, _>("exports"), 1);
    assert_eq!(counts.get::<i64, _>("forks"), 1);

    let forged_projection = setup_runtime.block_on(
        sqlx::query(
            r#"
            INSERT INTO public.campaign_exports (
                export_id, campaign_id, requested_by, audience, state,
                requested_at, version, visibility_label, visibility_subject,
                provenance_kind, provenance_reference, provenance_recorded_by,
                last_event_sequence
            ) VALUES (
                'export_ar06_forged', $1, $2, 'CAMPAIGN_ARCHIVE', 'REQUESTED',
                now(), 1, 'keeper_only', 'not_applicable',
                'human_keeper_statement', 'forged', $2, $3
            )
            "#,
        )
        .bind(CAMPAIGN_ID)
        .bind(KEEPER_ID)
        .bind(action_sequence)
        .execute(&api_pool),
    );
    assert!(
        forged_projection.is_err(),
        "trpg_api_login must not forge a projection without canonical capability"
    );

    setup_runtime.block_on(async {
        api_pool.close().await;
        canonical_pool.close().await;
    });
    drop(application);
    let _ = std::fs::remove_file(&audit_path);
    let _ = std::fs::remove_file(audit_path.with_extension("jsonl.head"));
}
