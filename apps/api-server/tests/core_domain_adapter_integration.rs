use std::env;
use std::str::FromStr;
use std::sync::Arc;

use api_server::core_domain::RepositoryCampaignCharacterPort;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::PgPool;
use trpg_api::api_contracts::{
    AcceptInviteApiRequest, ApiCommandFields, AuthoritySnapshotApiRequest, CampaignCharacterApi,
    CreateCampaignApiRequest, CreateCharacterApiRequest, IssueInviteApiRequest,
};
use trpg_data_eventing::event_store_sqlx_outbox_projection::PostgresCanonicalStore;
use trpg_data_eventing::persistence_postgresql::CoreDomainRepository;
use trpg_shared_kernel::{
    Actor, ActorRole, AuthenticatedCommandContext, AuthorityContract, AuthorityMode,
    CanonicalPolicyAudit, ResourceRef, WorkloadRole,
};

const INTEGRITY_KEY: &[u8; 32] = &[0x58; 32];
const PAYLOAD_KEY: &[u8; 32] = &[0x69; 32];
const CAMPAIGN_ID: &str = "campaign_p06_production_adapter";
const KEEPER_ID: &str = "keeper_p06_production_adapter";
const PLAYER_ID: &str = "player_p06_production_adapter";
const NOW_MS: u64 = 2_000_000_100_000;

async fn reset_database(url: &str, expected_database: &str, witness: bool) -> PgPool {
    assert_eq!(
        env::var("P06_ALLOW_DATABASE_RESET").as_deref(),
        Ok("1"),
        "production-adapter integration requires explicit dedicated DB reset authorization"
    );
    let options = PgConnectOptions::from_str(url).expect("valid P06 PostgreSQL URL");
    assert!(matches!(
        options.get_host(),
        "localhost" | "127.0.0.1" | "::1"
    ));
    assert_eq!(options.get_database(), Some(expected_database));
    let pool = PgPoolOptions::new()
        .max_connections(12)
        .connect_with(options)
        .await
        .expect("connect dedicated P06 PostgreSQL database");
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
        .expect("reset dedicated production-adapter schemas");
    pool
}

fn command(suffix: &str, expected_version: i64) -> ApiCommandFields {
    ApiCommandFields {
        command_id: format!("command_{suffix}"),
        idempotency_key: format!("idempotency_{suffix}"),
        expected_version,
        correlation_id: format!("correlation_{suffix}"),
        causation_id: format!("causation_{suffix}"),
        trace_id: format!("trace_{suffix}"),
    }
}

fn context(
    contract: &AuthorityContract,
    actor_id: &str,
    actor_role: ActorRole,
    resource_type: &str,
    resource_id: &str,
    suffix: &str,
) -> trpg_api::api_contracts::AuthorizedCoreApiContext {
    // This fixture proves the production adapter binding only. The required
    // trpg-api integration test separately obtains these audit fields from
    // real OpenFGA and OPA decisions.
    let resource = ResourceRef::new(CAMPAIGN_ID, resource_type, resource_id).unwrap();
    let requesting = AuthenticatedCommandContext::new(
        Actor::authenticated_user(actor_id, actor_role, format!("session_{suffix}")).unwrap(),
        resource.clone(),
        contract.binding().unwrap(),
        format!("trace_request_{suffix}"),
        NOW_MS,
        NOW_MS + 60_000,
    )
    .unwrap();
    let workflow_actor_id = "workflow_p06_production_adapter";
    let workflow = AuthenticatedCommandContext::new(
        Actor::verified_workload(workflow_actor_id, WorkloadRole::WorkflowEngine).unwrap(),
        resource,
        contract.binding().unwrap(),
        format!("trace_workflow_{suffix}"),
        NOW_MS,
        NOW_MS + 60_000,
    )
    .unwrap();
    trpg_api::api_contracts::AuthorizedCoreApiContext::from_authenticated_contexts(
        requesting,
        workflow,
        contract,
        CanonicalPolicyAudit {
            actor_id: workflow_actor_id.to_owned(),
            actor_origin: "workload".to_owned(),
            authentication_reference: workflow_actor_id.to_owned(),
            resource_type: resource_type.to_owned(),
            resource_id: resource_id.to_owned(),
            action: "write_official_state".to_owned(),
            requested_role: "workflow".to_owned(),
            openfga_decision_id: format!("lower_layer_fixture_openfga_{suffix}"),
            openfga_policy_revision: "lower-layer-production-adapter-fixture-v1".to_owned(),
            opa_decision_id: format!("lower_layer_fixture_opa_{suffix}"),
            opa_policy_revision: "lower-layer-production-adapter-fixture-v1".to_owned(),
        },
    )
    .unwrap()
}

fn character_sheet_json() -> String {
    serde_json::json!({
        "name": "Ada Mercer",
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn production_adapter_executes_real_campaign_invite_and_character_repository_chain() {
    let primary_url = env::var("P06_DATABASE_URL").expect("P06_DATABASE_URL is required");
    let witness_url =
        env::var("P06_WITNESS_DATABASE_URL").expect("P06_WITNESS_DATABASE_URL is required");
    let primary_database = env::var("P06_RESET_DATABASE").unwrap();
    let witness_database = env::var("P06_WITNESS_RESET_DATABASE").unwrap();
    let primary = reset_database(&primary_url, &primary_database, false).await;
    let witness = reset_database(&witness_url, &witness_database, true).await;
    witness.close().await;

    let store = PostgresCanonicalStore::connect(
        &primary_url,
        &witness_url,
        "p06-production-adapter-integrity-key",
        INTEGRITY_KEY,
        "p06-production-adapter-payload-key",
        PAYLOAD_KEY,
    )
    .await
    .expect("connect canonical primary and independent witness");
    store
        .prepare_for_service()
        .await
        .expect("apply P06 migrations");
    for (user_id, login) in [
        (KEEPER_ID, "keeper-p06-production-adapter"),
        (PLAYER_ID, "player-p06-production-adapter"),
    ] {
        sqlx::query(
            "INSERT INTO public.users \
             (user_id, login_normalized, password_hash, global_role) \
             VALUES ($1, $2, 'not-used-by-adapter-test', 'USER')",
        )
        .bind(user_id)
        .bind(login)
        .execute(&primary)
        .await
        .unwrap();
    }

    let contract = trpg_test_support::authority_contract_with_owner(
        CAMPAIGN_ID,
        AuthorityMode::HumanKp,
        KEEPER_ID,
        1,
    )
    .unwrap();
    let api = CampaignCharacterApi::new(Arc::new(RepositoryCampaignCharacterPort::new(
        CoreDomainRepository::new(primary.clone(), store),
    )));
    let keeper_campaign = context(
        &contract,
        KEEPER_ID,
        ActorRole::HumanKeeper,
        "campaign",
        CAMPAIGN_ID,
        "campaign_create",
    );
    api.create_campaign(
        &keeper_campaign,
        &CreateCampaignApiRequest {
            command: command("production_adapter_campaign", 0),
            campaign_id: CAMPAIGN_ID.to_owned(),
            owner_user_id: KEEPER_ID.to_owned(),
            title: "P06 production adapter campaign".to_owned(),
            room_id: "room_p06_production_adapter".to_owned(),
            room_name: "Production adapter table".to_owned(),
            created_at_unix_ms: NOW_MS,
            authority: AuthoritySnapshotApiRequest {
                contract_id: contract.contract_id().to_string(),
                authority_mode: "HUMAN_KP".to_owned(),
                authority_owner: KEEPER_ID.to_owned(),
                ruleset_version: "coc7-1".to_owned(),
                house_rules_version: "none-1".to_owned(),
                scenario_version: "tutorial-1".to_owned(),
                prompt_version: "p06-1".to_owned(),
                agent_pack_version: "none-1".to_owned(),
                tool_schema_version: "tools-1".to_owned(),
                safety_profile_version: "safety-1".to_owned(),
                ai_provider_snapshot: "not_applicable".to_owned(),
                model_route_snapshot: "not_applicable".to_owned(),
                character_sheet_template_version: "coc7-sheet-1".to_owned(),
            },
        },
    )
    .await
    .expect("production adapter creates Campaign and locked Authority");

    let invite_id = "invite_p06_production_adapter";
    let keeper_invite = context(
        &contract,
        KEEPER_ID,
        ActorRole::HumanKeeper,
        "campaign_invite",
        invite_id,
        "invite_issue",
    );
    let invite_request = IssueInviteApiRequest {
        command: command("production_adapter_invite", 0),
        campaign_id: CAMPAIGN_ID.to_owned(),
        invite_id: invite_id.to_owned(),
        invited_user_id: PLAYER_ID.to_owned(),
        role: "PLAYER".to_owned(),
        expires_at_unix_ms: NOW_MS + 60_000,
        now_unix_ms: NOW_MS,
    };
    let issued = api
        .issue_invite(&keeper_invite, &invite_request)
        .await
        .expect("production adapter issues invite");
    let issued_retry = api
        .issue_invite(&keeper_invite, &invite_request)
        .await
        .expect("production adapter exact invite retry");
    assert_eq!(issued_retry.raw_token, issued.raw_token);
    assert_eq!(
        issued_retry.receipt.last_event_sequence,
        issued.receipt.last_event_sequence
    );

    let player_invite = context(
        &contract,
        PLAYER_ID,
        ActorRole::Investigator,
        "campaign_invite",
        invite_id,
        "invite_accept",
    );
    api.accept_invite(
        &player_invite,
        &AcceptInviteApiRequest {
            command: command("production_adapter_accept", 1),
            campaign_id: CAMPAIGN_ID.to_owned(),
            invite_id: invite_id.to_owned(),
            accepting_user_id: PLAYER_ID.to_owned(),
            raw_token: issued.raw_token,
            accepted_at_unix_ms: NOW_MS + 1_000,
        },
    )
    .await
    .expect("production adapter accepts invite into durable Membership");

    let character_id = "character_p06_production_adapter";
    let player_character = context(
        &contract,
        PLAYER_ID,
        ActorRole::Investigator,
        "character",
        character_id,
        "character_create",
    );
    api.create_character(
        &player_character,
        &CreateCharacterApiRequest {
            command: command("production_adapter_character", 0),
            campaign_id: CAMPAIGN_ID.to_owned(),
            character_id: character_id.to_owned(),
            owner_user_id: PLAYER_ID.to_owned(),
            display_name: "Ada Mercer".to_owned(),
            sheet_version_id: "sheet_p06_production_adapter_v1".to_owned(),
            sheet_json: character_sheet_json(),
        },
    )
    .await
    .expect("production adapter validates and persists a COC7 Character");

    let event_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM public.event_store WHERE campaign_id = $1")
            .bind(CAMPAIGN_ID)
            .fetch_one(&primary)
            .await
            .unwrap();
    assert_eq!(
        event_count, 4,
        "exact invite retry must not append a duplicate canonical event"
    );
}
