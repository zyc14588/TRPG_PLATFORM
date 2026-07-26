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

#[path = "../../../apps/api-server/src/player_action.rs"]
mod production_player_action;

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

async fn seed_tutorial(repository: &CoreDomainRepository, primary: &PgPool) {
    for (id, login) in [
        (KEEPER_ID, "keeper-p07-http"),
        (PLAYER_ID, "player-p07-http"),
    ] {
        sqlx::query(
            "INSERT INTO public.users \
             (user_id, login_normalized, password_hash, global_role) \
             VALUES ($1, $2, 'not-used-by-p07-http-test', 'USER')",
        )
        .bind(id)
        .bind(login)
        .execute(primary)
        .await
        .unwrap();
    }
    sqlx::query(
        "INSERT INTO public.users \
         (user_id, login_normalized, password_hash, global_role) \
         VALUES ($1, $2, 'not-used-by-p07-http-test', 'USER')",
    )
    .bind(OTHER_KEEPER_ID)
    .bind("keeper-p07-http-other")
    .execute(primary)
    .await
    .unwrap();

    repository
        .create_campaign(
            &seed_metadata(
                KEEPER_ID,
                "human_keeper",
                CAMPAIGN_ID,
                "campaign",
                0,
                "p07_http_campaign",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &CreateCampaignRequest {
                campaign_id: CAMPAIGN_ID.to_owned(),
                owner_user_id: KEEPER_ID.to_owned(),
                title: "P07 HTTP tutorial".to_owned(),
                room_id: "room_p07_http".to_owned(),
                room_name: "P07 HTTP table".to_owned(),
                created_at_unix_ms: NOW_MS,
                authority: authority_snapshot(),
            },
        )
        .await
        .unwrap();
    let invite = repository
        .issue_invite(
            &seed_metadata(
                KEEPER_ID,
                "human_keeper",
                "invite_p07_http",
                "campaign_invite",
                0,
                "p07_http_invite",
                "private_to_player",
                PLAYER_ID,
                "human_keeper_statement",
            ),
            &IssueInviteRequest {
                invite_id: "invite_p07_http".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                invited_user_id: PLAYER_ID.to_owned(),
                role: MembershipRole::Player,
                expires_at_unix_ms: NOW_MS + 60_000,
                now_unix_ms: NOW_MS,
            },
        )
        .await
        .unwrap();
    repository
        .accept_invite(
            &seed_metadata(
                PLAYER_ID,
                "investigator",
                "invite_p07_http",
                "campaign_invite",
                1,
                "p07_http_accept",
                "private_to_player",
                PLAYER_ID,
                "user_statement",
            ),
            &AcceptInviteRequest {
                campaign_id: CAMPAIGN_ID.to_owned(),
                invite_id: "invite_p07_http".to_owned(),
                accepting_user_id: PLAYER_ID.to_owned(),
                raw_token: invite.raw_token,
                accepted_at_unix_ms: NOW_MS + 1_000,
            },
        )
        .await
        .unwrap();
    repository
        .create_character(
            &seed_metadata(
                PLAYER_ID,
                "investigator",
                CHARACTER_ID,
                "character",
                0,
                "p07_http_character",
                "private_to_player",
                PLAYER_ID,
                "user_statement",
            ),
            &CreateCharacterRequest {
                character_id: CHARACTER_ID.to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                owner_user_id: PLAYER_ID.to_owned(),
                display_name: "Evelyn Hart".to_owned(),
                sheet_version_id: "sheet_p07_http_v1".to_owned(),
                sheet_json: character_sheet(),
            },
        )
        .await
        .unwrap();
    repository
        .submit_character(
            &seed_metadata(
                PLAYER_ID,
                "investigator",
                CHARACTER_ID,
                "character",
                1,
                "p07_http_character_submit",
                "private_to_player",
                PLAYER_ID,
                "user_statement",
            ),
            CAMPAIGN_ID,
            CHARACTER_ID,
        )
        .await
        .unwrap();
    repository
        .approve_character_initial_version(
            &seed_metadata(
                KEEPER_ID,
                "human_keeper",
                CHARACTER_ID,
                "character",
                2,
                "p07_http_character_approve",
                "private_to_player",
                PLAYER_ID,
                "human_keeper_statement",
            ),
            CAMPAIGN_ID,
            CHARACTER_ID,
        )
        .await
        .unwrap();
    let tutorial = parse_scenario_yaml(include_str!(
        "../../../fixtures/scenarios/tutorial_mist_archive.scenario.yaml"
    ))
    .unwrap();
    repository
        .import_scenario(
            &seed_metadata(
                KEEPER_ID,
                "human_keeper",
                "scenario_p07_http",
                "scenario",
                0,
                "p07_http_scenario",
                "keeper_only",
                "not_applicable",
                "imported_source",
            ),
            &ImportScenarioRequest {
                scenario_id: "scenario_p07_http".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                ruleset_id: tutorial.ruleset_id,
                format_version: tutorial.format_version,
                content_hash: tutorial.content_hash,
                document_json: tutorial.canonical_json,
            },
        )
        .await
        .unwrap();
    repository
        .start_session(
            &seed_metadata(
                KEEPER_ID,
                "human_keeper",
                "session_p07_http",
                "session",
                0,
                "p07_http_session",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &StartSessionRequest {
                session_id: "session_p07_http".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                room_id: "room_p07_http".to_owned(),
                scenario_id: "scenario_p07_http".to_owned(),
                scene_id: "scene_p07_http".to_owned(),
                scene_key: "scene_archive_front".to_owned(),
                scene_name: "灰港市政档案室前厅".to_owned(),
                started_at_unix_ms: NOW_MS + 2_000,
            },
        )
        .await
        .unwrap();
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

impl HttpPlayerActionApplication {
    fn new(
        identity: IdentityService,
        policy: OpenFgaOpaPolicyAdapter,
        audit: FileAuditLog,
        port: RepositoryPlayerActionPort,
    ) -> Self {
        let authorizer = FormalCommitAuthorizer::new(
            identity.verifier(),
            policy,
            FormalCommitAudit::from_file_log(audit),
        );
        Self {
            identity: Arc::new(Mutex::new(identity)),
            authorizer,
            runtime: Arc::new(Mutex::new(tokio::runtime::Runtime::new().unwrap())),
            port,
        }
    }

    fn handle(&self, request: &HttpRequest) -> Option<HttpResponse> {
        let segments = request
            .path
            .trim_matches('/')
            .split('/')
            .collect::<Vec<_>>();
        match (request.method.as_str(), segments.as_slice()) {
            ("POST", ["campaigns", campaign_id, "player-actions"]) => {
                Some(self.submit(request, campaign_id))
            }
            ("POST", ["campaigns", campaign_id, "player-actions", action_id, "confirm"]) => {
                Some(self.confirm(request, campaign_id, action_id))
            }
            _ => None,
        }
    }

    fn submit(&self, request: &HttpRequest, campaign_id: &str) -> HttpResponse {
        let body: SubmitPlayerActionApiRequest = match serde_json::from_slice(&request.body) {
            Ok(body) => body,
            Err(_) => return HttpResponse::json(400, json!({"error": "INVALID_JSON_BODY"})),
        };
        if body.campaign_id != campaign_id {
            return HttpResponse::json(400, json!({"error": "PLAYER_ACTION_PATH_BODY_MISMATCH"}));
        }
        let context =
            match self.authorized_context(request, campaign_id, &body.action_id, &body.command) {
                Ok(context) => context,
                Err(response) => return response,
            };
        let api = PlayerActionApi::new(Arc::new(self.port.clone()));
        let result = self
            .runtime
            .lock()
            .expect("P07 HTTP runtime lock")
            .block_on(api.submit(&context, &body));
        match result {
            Ok(receipt) => HttpResponse::json(
                202,
                json!({
                    "first_event_sequence": receipt.first_event_sequence,
                    "last_event_sequence": receipt.last_event_sequence,
                    "aggregate_version": receipt.aggregate_version,
                    "state": receipt.state,
                    "realtime_delta_id": receipt.realtime_delta_id,
                }),
            ),
            Err(error) => core_api_error(error),
        }
    }

    fn confirm(&self, request: &HttpRequest, campaign_id: &str, action_id: &str) -> HttpResponse {
        let body: ConfirmPlayerActionApiRequest = match serde_json::from_slice(&request.body) {
            Ok(body) => body,
            Err(_) => return HttpResponse::json(400, json!({"error": "INVALID_JSON_BODY"})),
        };
        if body.campaign_id != campaign_id || body.action_id != action_id {
            return HttpResponse::json(400, json!({"error": "PLAYER_ACTION_PATH_BODY_MISMATCH"}));
        }
        let context = match self.authorized_context(request, campaign_id, action_id, &body.command)
        {
            Ok(context) => context,
            Err(response) => return response,
        };
        let api = PlayerActionApi::new(Arc::new(self.port.clone()));
        let result = self
            .runtime
            .lock()
            .expect("P07 HTTP runtime lock")
            .block_on(api.confirm(&context, &body));
        match result {
            Ok(receipt) => HttpResponse::json(
                200,
                json!({
                    "first_event_sequence": receipt.first_event_sequence,
                    "last_event_sequence": receipt.last_event_sequence,
                    "aggregate_version": receipt.aggregate_version,
                    "state": receipt.state,
                    "realtime_delta_id": receipt.realtime_delta_id,
                }),
            ),
            Err(error) => core_api_error(error),
        }
    }

    fn authorized_context(
        &self,
        request: &HttpRequest,
        campaign_id: &str,
        action_id: &str,
        command: &ApiCommandFields,
    ) -> Result<AuthorizedCoreApiContext, HttpResponse> {
        let now = now_unix_ms();
        let token = request
            .header("authorization")
            .and_then(|value| value.strip_prefix("Bearer "))
            .filter(|value| !value.is_empty())
            .ok_or_else(|| HttpResponse::json(401, json!({"error": "AUTHENTICATION_REQUIRED"})))?;
        let campaign = EntityId::new(campaign_id)
            .map_err(|_| HttpResponse::json(400, json!({"error": "INVALID_ENTITY_ID"})))?;
        let resource = ResourceRef::new(campaign_id, "player_action", action_id)
            .map_err(|_| HttpResponse::json(400, json!({"error": "INVALID_ENTITY_ID"})))?;
        let (
            requesting_authentication,
            requesting_actor,
            workflow_authentication,
            workflow_actor,
            authority,
        ) = {
            let mut identity = self
                .identity
                .lock()
                .map_err(|_| HttpResponse::json(500, json!({"error": "IDENTITY_LOCK_FAILED"})))?;
            let requesting_authentication = identity
                .authenticate_session(Some(token), now)
                .map_err(|_| {
                    HttpResponse::json(401, json!({"error": "AUTHENTICATION_REQUIRED"}))
                })?;
            let requesting_actor = identity
                .command_actor(&requesting_authentication, &campaign, now)
                .map_err(|_| HttpResponse::json(403, json!({"error": "CAMPAIGN_FORBIDDEN"})))?;
            let expires_at = now
                .checked_add(60_000)
                .ok_or_else(|| HttpResponse::json(500, json!({"error": "CLOCK_OVERFLOW"})))?;
            let credential = identity
                .issue_workload_credential(
                    "api_player_action_workflow",
                    WorkloadRole::WorkflowEngine,
                    now,
                    expires_at,
                )
                .map_err(|_| {
                    HttpResponse::json(503, json!({"error": "WORKFLOW_IDENTITY_UNAVAILABLE"}))
                })?;
            let workflow_authentication = identity
                .authenticate_workload(&credential, now)
                .map_err(|_| {
                    HttpResponse::json(503, json!({"error": "WORKFLOW_IDENTITY_UNAVAILABLE"}))
                })?;
            let workflow_actor = identity
                .command_actor(&workflow_authentication, &campaign, now)
                .map_err(|_| {
                    HttpResponse::json(503, json!({"error": "WORKFLOW_IDENTITY_UNAVAILABLE"}))
                })?;
            let authority = identity
                .authority_contract(&campaign)
                .map_err(|_| HttpResponse::json(503, json!({"error": "AUTHORITY_UNAVAILABLE"})))?
                .ok_or_else(|| {
                    HttpResponse::json(404, json!({"error": "AUTHORITY_CONTRACT_NOT_FOUND"}))
                })?;
            (
                requesting_authentication,
                requesting_actor,
                workflow_authentication,
                workflow_actor,
                authority,
            )
        };
        let authority_binding = authority
            .binding()
            .map_err(|_| HttpResponse::json(403, json!({"error": "AUTHORITY_CONTRACT_INVALID"})))?;
        let requesting_context = AuthenticatedCommandContext::new(
            requesting_actor.clone(),
            resource.clone(),
            authority_binding.clone(),
            command.trace_id.clone(),
            requesting_authentication.authenticated_at_unix_ms(),
            requesting_authentication.expires_at_unix_ms(),
        )
        .map_err(|_| HttpResponse::json(403, json!({"error": "REQUEST_CONTEXT_INVALID"})))?;
        let workflow_context = AuthenticatedCommandContext::new(
            workflow_actor,
            resource,
            authority_binding,
            command.trace_id.clone(),
            workflow_authentication.authenticated_at_unix_ms(),
            workflow_authentication.expires_at_unix_ms(),
        )
        .map_err(|_| HttpResponse::json(403, json!({"error": "WORKFLOW_CONTEXT_INVALID"})))?;
        let expected_version = u64::try_from(command.expected_version).map_err(|_| {
            HttpResponse::json(
                400,
                json!({"error": "PLAYER_ACTION_EXPECTED_VERSION_INVALID"}),
            )
        })?;
        let provenance_kind =
            if requesting_actor.role() == &trpg_shared_kernel::ActorRole::HumanKeeper {
                ProvenanceKind::HumanKeeperStatement
            } else {
                ProvenanceKind::UserStatement
            };
        let policy_command = CommandEnvelope::new(
            (),
            CommandMetadata {
                command_id: EntityId::new(&command.command_id).map_err(|_| {
                    HttpResponse::json(400, json!({"error": "PLAYER_ACTION_COMMAND_ID_INVALID"}))
                })?,
                idempotency_key: command.idempotency_key.clone(),
                expected_version,
                authority_mode: authority.mode().clone(),
                visibility: Visibility::new(VisibilityLabel::PartyVisible),
                fact_provenance: FactProvenance::new(
                    provenance_kind,
                    &command.command_id,
                    requesting_actor.id().as_str(),
                )
                .map_err(|_| {
                    HttpResponse::json(400, json!({"error": "PLAYER_ACTION_PROVENANCE_INVALID"}))
                })?,
                correlation_id: EntityId::new(&command.correlation_id).map_err(|_| {
                    HttpResponse::json(
                        400,
                        json!({"error": "PLAYER_ACTION_CORRELATION_ID_INVALID"}),
                    )
                })?,
                causation_id: EntityId::new(&command.causation_id).map_err(|_| {
                    HttpResponse::json(400, json!({"error": "PLAYER_ACTION_CAUSATION_ID_INVALID"}))
                })?,
                write_path: FormalWritePath::WorkflowDecision,
                authenticated_context: workflow_context.clone(),
            },
        );
        let authorization = self
            .authorizer
            .authorize(
                &workflow_authentication,
                None,
                &policy_command,
                "workflow",
                now,
            )
            .map_err(|_| HttpResponse::json(403, json!({"error": "POLICY_DENIED"})))?;
        AuthorizedCoreApiContext::from_authenticated_contexts(
            requesting_context,
            workflow_context,
            &authority,
            authorization.canonical_audit().clone(),
        )
        .map_err(core_api_error)
    }
}

fn core_api_error(error: CoreApiError) -> HttpResponse {
    HttpResponse::json(error.status_code(), json!({"error": error.to_string()}))
}

fn exchange(application: HttpPlayerActionApplication, request: String) -> (u16, Value) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let request = read_request(&mut stream);
        let response = application.handle(&request).unwrap_or_else(|| {
            trpg_contracts::HttpResponse::json(404, json!({"error": "NOT_FOUND"}))
        });
        let body = response.body.to_string();
        write!(
            stream,
            "HTTP/1.1 {} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            response.status,
            body.len(),
            body
        )
        .unwrap();
    });
    let mut stream = TcpStream::connect(address).unwrap();
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    server.join().unwrap();
    let (headers, body) = response.split_once("\r\n\r\n").unwrap();
    let status = headers
        .lines()
        .next()
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    (status, serde_json::from_str(body).unwrap())
}

fn read_request(stream: &mut TcpStream) -> HttpRequest {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 4096];
    let (boundary, content_length) = loop {
        let count = stream.read(&mut buffer).unwrap();
        assert!(count > 0, "client closed before sending a complete request");
        bytes.extend_from_slice(&buffer[..count]);
        if let Some(boundary) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&bytes[..boundary]);
            let content_length = headers
                .lines()
                .filter_map(|line| line.split_once(':'))
                .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                .map(|(_, value)| value.trim().parse::<usize>().unwrap())
                .unwrap_or(0);
            if bytes.len() >= boundary + 4 + content_length {
                break (boundary, content_length);
            }
        }
    };
    let headers = String::from_utf8_lossy(&bytes[..boundary]);
    let mut lines = headers.lines();
    let request_line = lines.next().unwrap().split_whitespace().collect::<Vec<_>>();
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.to_ascii_lowercase(), value.trim().to_owned()))
        .collect::<HashMap<_, _>>();
    HttpRequest {
        method: request_line[0].to_owned(),
        path: request_line[1].to_owned(),
        headers,
        body: bytes[boundary + 4..boundary + 4 + content_length].to_vec(),
    }
}

fn json_request(method: &str, path: &str, token: Option<&str>, body: Value) -> String {
    let body = body.to_string();
    let authorization = token.map_or_else(String::new, |token| {
        format!("Authorization: Bearer {token}\r\n")
    });
    format!(
        "{method} {path} HTTP/1.1\r\nHost: localhost\r\n{authorization}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

fn command(suffix: &str, expected_version: i64) -> Value {
    json!({
        "command_id": format!("command_{suffix}"),
        "idempotency_key": format!("idempotency_{suffix}"),
        "expected_version": expected_version,
        "correlation_id": format!("correlation_{suffix}"),
        "causation_id": format!("causation_{suffix}"),
        "trace_id": format!("trace_{suffix}")
    })
}

#[test]
fn player_action_http_path_authenticates_executes_rules_and_commits_atomically() {
    let primary_url = env::var("P07_DATABASE_URL").expect("P07_DATABASE_URL required");
    let witness_url =
        env::var("P07_WITNESS_DATABASE_URL").expect("P07_WITNESS_DATABASE_URL required");
    let nats_url = env::var("P07_NATS_URL").expect("P07_NATS_URL required");
    let primary_name = env::var("P07_RESET_DATABASE").unwrap();
    let witness_name = env::var("P07_WITNESS_RESET_DATABASE").unwrap();
    let setup_runtime = tokio::runtime::Runtime::new().unwrap();
    let primary = setup_runtime.block_on(reset_database(&primary_url, &primary_name, false));
    let witness = setup_runtime.block_on(reset_database(&witness_url, &witness_name, true));
    setup_runtime.block_on(witness.close());
    let store = setup_runtime
        .block_on(PostgresCanonicalStore::connect(
            &primary_url,
            &witness_url,
            "p07-http-integrity-key",
            INTEGRITY_KEY,
            "p07-http-payload-key",
            PAYLOAD_KEY,
        ))
        .unwrap();
    setup_runtime
        .block_on(store.prepare_for_service())
        .expect("P07 migrations apply to empty primary and witness databases");
    let canonical = store.clone();
    let repository = CoreDomainRepository::new(primary.clone(), store);
    setup_runtime.block_on(seed_tutorial(&repository, &primary));
    let (publisher, mut realtime_messages) = setup_runtime.block_on(async {
        let nats = async_nats::connect(&nats_url)
            .await
            .expect("connect to dedicated P07 JetStream");
        let jetstream = async_nats::jetstream::new(nats.clone());
        let _ = jetstream.delete_stream("TRPG_CANONICAL_EVENTS").await;
        let messages = nats
            .subscribe("trpg.events.appended.>")
            .await
            .expect("subscribe to P07 canonical realtime events");
        nats.flush()
            .await
            .expect("activate P07 realtime subscription");
        let publisher =
            JetStreamOutboxPublisher::connect(canonical, &nats_url, "p07-http-realtime", None)
                .await
                .expect("construct production P07 Outbox publisher");
        publisher
            .ensure_stream()
            .await
            .expect("create canonical P07 JetStream");
        (publisher, messages)
    });

    let audit_directory = tempfile::Builder::new()
        .prefix("trpg-p07-http-audit-")
        .tempdir()
        .unwrap();
    let audit_path = audit_directory.path().join("formal-audit.jsonl");
    let audit = FileAuditLog::open(&audit_path, "p07-http-audit-v1", &[0x69; 32]).unwrap();
    let (identity, tokens) = identity(now_unix_ms());
    let application = HttpPlayerActionApplication::new(
        identity,
        policy(),
        audit,
        RepositoryPlayerActionPort::new(repository),
    );
    let player_token = tokens.player;
    let keeper_token = tokens.keeper;
    let other_keeper_token = tokens.other_keeper;

    let invalid_action = "action_p07_http_client_dice";
    let event_count_before: i64 = setup_runtime.block_on(async {
        sqlx::query_scalar("SELECT count(*) FROM public.event_store")
            .fetch_one(&primary)
            .await
            .unwrap()
    });
    let (status, _) = exchange(
        application.clone(),
        json_request(
            "POST",
            &format!("/campaigns/{CAMPAIGN_ID}/player-actions"),
            Some(&player_token),
            json!({
                "command": command("p07_http_client_dice", 0),
                "campaign_id": CAMPAIGN_ID,
                "action_id": invalid_action,
                "character_id": CHARACTER_ID,
                "scene_id": "scene_p07_http",
                "submitted_at_unix_ms": NOW_MS + 3_000,
                "intent": {
                    "kind": "INVESTIGATION",
                    "skill_name": "Library Use",
                    "clue_id": "clue_wrong_signature",
                    "clue_importance": "CORE",
                    "adjustment": "NONE",
                    "roll": 1
                }
            }),
        ),
    );
    assert_eq!(status, 400, "client-supplied dice must be rejected");
    let event_count_after: i64 = setup_runtime.block_on(async {
        sqlx::query_scalar("SELECT count(*) FROM public.event_store")
            .fetch_one(&primary)
            .await
            .unwrap()
    });
    assert_eq!(event_count_after, event_count_before);

    let submission = json!({
        "command": command("p07_http_submit", 0),
        "campaign_id": CAMPAIGN_ID,
        "action_id": ACTION_ID,
        "character_id": CHARACTER_ID,
        "scene_id": "scene_p07_http",
        "submitted_at_unix_ms": NOW_MS + 3_000,
        "intent": {
            "kind": "INVESTIGATION",
            "skill_name": "Library Use",
            "clue_id": "clue_wrong_signature",
            "clue_importance": "CORE",
            "adjustment": "NONE"
        }
    });
    let (status, submitted) = exchange(
        application.clone(),
        json_request(
            "POST",
            &format!("/campaigns/{CAMPAIGN_ID}/player-actions"),
            Some(&player_token),
            submission,
        ),
    );
    assert_eq!(status, 202);
    assert_eq!(submitted["state"], "AWAITING_HUMAN_CONFIRMATION");
    assert!(submitted.get("rolled_value").is_none());
    let dice_before_confirmation: i64 = setup_runtime.block_on(async {
        sqlx::query_scalar("SELECT count(*) FROM public.dice_rolls")
            .fetch_one(&primary)
            .await
            .unwrap()
    });
    assert_eq!(dice_before_confirmation, 0);

    let (status, _) = exchange(
        application.clone(),
        json_request(
            "POST",
            &format!("/campaigns/{CAMPAIGN_ID}/player-actions/{ACTION_ID}/confirm"),
            Some(&other_keeper_token),
            json!({
                "command": command("p07_http_non_owner_confirm", 1),
                "campaign_id": CAMPAIGN_ID,
                "action_id": ACTION_ID,
                "resolved_at_unix_ms": NOW_MS + 4_000
            }),
        ),
    );
    assert_eq!(status, 403);
    let dice_after_denial: i64 = setup_runtime.block_on(async {
        sqlx::query_scalar("SELECT count(*) FROM public.dice_rolls")
            .fetch_one(&primary)
            .await
            .unwrap()
    });
    assert_eq!(dice_after_denial, 0);

    let confirmation = json!({
        "command": command("p07_http_confirm", 1),
        "campaign_id": CAMPAIGN_ID,
        "action_id": ACTION_ID,
        "resolved_at_unix_ms": NOW_MS + 4_000
    });
    let (status, resolved) = exchange(
        application.clone(),
        json_request(
            "POST",
            &format!("/campaigns/{CAMPAIGN_ID}/player-actions/{ACTION_ID}/confirm"),
            Some(&keeper_token),
            confirmation.clone(),
        ),
    );
    assert_eq!(status, 200);
    assert_eq!(resolved["state"], "RESOLVED");
    assert!(resolved.get("rolled_value").is_none());
    let first_sequence = resolved["first_event_sequence"].as_i64().unwrap();
    let last_sequence = resolved["last_event_sequence"].as_i64().unwrap();
    assert_eq!(
        resolved["realtime_delta_id"],
        format!("delta_player_action_{last_sequence}")
    );

    let realtime_binding = setup_runtime.block_on(async {
        sqlx::query(
            "SELECT o.nats_subject, o.visibility_label, o.visibility_subject, \
                    o.correlation_id, e.trace_id, e.fact_provenance_kind, \
                    e.fact_provenance_reference \
               FROM public.event_outbox o \
               JOIN public.event_store e ON e.sequence = o.event_sequence \
              WHERE o.event_sequence = $1",
        )
        .bind(last_sequence)
        .fetch_one(&primary)
        .await
        .unwrap()
    });
    assert_eq!(
        realtime_binding.get::<String, _>("nats_subject"),
        "trpg.events.appended"
    );
    assert_eq!(
        realtime_binding.get::<String, _>("visibility_label"),
        "party_visible"
    );
    assert_eq!(
        realtime_binding.get::<String, _>("visibility_subject"),
        "not_applicable"
    );
    assert_eq!(
        realtime_binding.get::<String, _>("correlation_id"),
        "correlation_p07_http_confirm"
    );
    assert_eq!(
        realtime_binding.get::<String, _>("trace_id"),
        "trace_p07_http_confirm"
    );
    assert_eq!(
        realtime_binding.get::<String, _>("fact_provenance_kind"),
        "human_keeper_statement"
    );
    assert_eq!(
        realtime_binding.get::<String, _>("fact_provenance_reference"),
        "command_p07_http_confirm"
    );

    let row = setup_runtime.block_on(async {
        sqlx::query(
            "SELECT d.target_value, d.rolled_value, d.random_source, \
                    c.importance, c.outcome, c.revealed_to_party \
             FROM public.dice_rolls d \
             JOIN public.clues c ON c.action_id = d.action_id \
             WHERE d.action_id = $1",
        )
        .bind(ACTION_ID)
        .fetch_one(&primary)
        .await
        .unwrap()
    });
    assert_eq!(row.get::<i16, _>("target_value"), 70);
    assert!((1..=100).contains(&row.get::<i16, _>("rolled_value")));
    assert_eq!(row.get::<String, _>("random_source"), "SERVER_OS_CSPRNG");
    assert_eq!(row.get::<String, _>("importance"), "CORE");
    assert!(matches!(
        row.get::<String, _>("outcome").as_str(),
        "REVEALED" | "REVEALED_WITH_COST"
    ));
    assert!(row.get::<bool, _>("revealed_to_party"));

    let (retry_status, retried) = exchange(
        application.clone(),
        json_request(
            "POST",
            &format!("/campaigns/{CAMPAIGN_ID}/player-actions/{ACTION_ID}/confirm"),
            Some(&keeper_token),
            confirmation,
        ),
    );
    assert_eq!(retry_status, 200);
    assert_eq!(retried["first_event_sequence"], first_sequence);
    assert_eq!(retried["last_event_sequence"], last_sequence);
    let counts = setup_runtime.block_on(async {
        sqlx::query(
            "SELECT \
               (SELECT count(*) FROM public.dice_rolls WHERE action_id = $1) AS dice, \
               (SELECT count(*) FROM public.decision_records WHERE action_id = $1) AS decisions, \
               (SELECT count(*) FROM public.clues WHERE action_id = $1) AS clues, \
               (SELECT count(*) FROM public.event_outbox \
                  WHERE commit_id = 'commit_command_p07_http_confirm') AS outbox",
        )
        .bind(ACTION_ID)
        .fetch_one(&primary)
        .await
        .unwrap()
    });
    assert_eq!(counts.get::<i64, _>("dice"), 1);
    assert_eq!(counts.get::<i64, _>("decisions"), 1);
    assert_eq!(counts.get::<i64, _>("clues"), 1);
    assert_eq!(counts.get::<i64, _>("outbox"), 4);

    let delivery = setup_runtime
        .block_on(publisher.publish_batch())
        .expect("publish P07 canonical Outbox through production JetStream adapter");
    assert_eq!(delivery.failed, 0);
    assert!(delivery.published >= 5);
    let mut delivered_action_delta = None;
    for _ in 0..delivery.published {
        let message = setup_runtime
            .block_on(async {
                tokio::time::timeout(Duration::from_secs(5), realtime_messages.next()).await
            })
            .expect("P07 realtime delivery timed out")
            .expect("P07 realtime subscription ended");
        let envelope: EventEnvelopeWire<Value> =
            serde_json::from_slice(&message.payload).expect("canonical realtime envelope");
        if i64::try_from(envelope.sequence).ok() == Some(last_sequence) {
            delivered_action_delta = Some(envelope);
        }
    }
    let delivered_action_delta =
        delivered_action_delta.expect("HTTP realtime_delta_id must resolve to a NATS envelope");
    assert_eq!(delivered_action_delta.stream_id, ACTION_ID);
    assert_eq!(delivered_action_delta.event_type, "DecisionCommitted");
    assert_eq!(delivered_action_delta.visibility_label, "party_visible");
    assert_eq!(
        delivered_action_delta.correlation_id,
        "correlation_p07_http_confirm"
    );
    assert_eq!(
        delivered_action_delta.provenance_reference,
        "command_p07_http_confirm"
    );
    let published: bool = setup_runtime.block_on(async {
        sqlx::query_scalar(
            "SELECT published_at IS NOT NULL FROM public.event_outbox WHERE event_sequence = $1",
        )
        .bind(last_sequence)
        .fetch_one(&primary)
        .await
        .unwrap()
    });
    assert!(
        published,
        "Realtime ACK must close the transactional Outbox row"
    );
    setup_runtime.block_on(async move {
        drop(realtime_messages);
        drop(publisher);
    });

    drop(application);
    setup_runtime.block_on(primary.close());
    drop(audit_directory);
}
