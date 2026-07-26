use std::collections::BTreeMap;
use std::env;
use std::str::FromStr;
use std::sync::Arc;

use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{PgPool, Row};
use trpg_api::api_contracts::{
    AcceptInviteApiRequest, ApiCommandFields, AuthoritySnapshotApiRequest,
    AuthorizedCoreApiContext, CampaignCharacterApi, CampaignCharacterCommandPort,
    CharacterTransitionApiRequest, CoreApiCommitReceipt, CoreApiError, CoreApiFuture,
    CreateCampaignApiRequest, CreateCharacterApiRequest, IssueInviteApiRequest,
    IssuedInviteApiResponse,
};
use trpg_data_eventing::event_store_sqlx_outbox_projection::{
    PolicyAuditDraft, PostgresCanonicalStore,
};
use trpg_data_eventing::persistence_postgresql::{
    AcceptInviteRequest, AuthorityContractSnapshot, CoreCommandMetadata, CoreDomainRepository,
    CoreDomainRepositoryError, CreateCampaignRequest, CreateCharacterRequest, IssueInviteRequest,
};
use trpg_domain_core::domain_entities_value_objects::MembershipRole;
use trpg_identity::{AuthenticationContext, CampaignRole, GlobalRole, WorkloadRole};
use trpg_ruleset_coc7::character_combat_san_chase::{Coc7CharacterSheet, Coc7Characteristics};
use trpg_security_governance::formal_commit_audit::{FormalCommitAudit, FormalCommitAuthorizer};
use trpg_security_governance::policy_adapter::{
    HttpPolicyEndpoint, OpenFgaOpaPolicyAdapter, PolicyBackend,
};
use trpg_security_governance::tamper_evident_audit::AuditDecision;
use trpg_shared_kernel::{
    AuthenticatedCommandContext, AuthorityMode, CommandEnvelope, CommandMetadata, EntityId,
    EventActorOriginWire, FactProvenance, FormalWritePath, ProvenanceKind, ResourceRef, Visibility,
};

const INTEGRITY_KEY: &[u8; 32] = &[0x7a; 32];
const PAYLOAD_KEY: &[u8; 32] = &[0x8b; 32];
const CAMPAIGN_ID: &str = "camp_human_archive";
const AUTHORITY_ID: &str = "authority_contract_camp_human_archive_1";
const KEEPER_ID: &str = "user_human_kp";
const PLAYER_ID: &str = "player_p06_api";
const NOW_MS: u64 = 2_200_000_000_000;

async fn reset_database(url: &str, expected_database: &str, witness: bool) -> PgPool {
    assert_eq!(
        env::var("P06_ALLOW_DATABASE_RESET").as_deref(),
        Ok("1"),
        "P06 API test requires explicit dedicated-database reset authorization"
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
        .expect("connect dedicated P06 API database");
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
        .expect("reset dedicated P06 API schemas");
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

struct RealFormalDecisionIssuer {
    identity: trpg_identity::IdentityService,
    workflow: trpg_identity::AuthenticationContext,
    keeper: AuthenticationContext,
    player: AuthenticationContext,
    contract: trpg_shared_kernel::AuthorityContract,
    authorizer: FormalCommitAuthorizer,
    audit: FormalCommitAudit,
    audit_path: std::path::PathBuf,
    _audit_directory: tempfile::TempDir,
}

impl RealFormalDecisionIssuer {
    fn new() -> Self {
        let contract = trpg_test_support::authority_contract_with_owner(
            CAMPAIGN_ID,
            AuthorityMode::HumanKp,
            KEEPER_ID,
            1,
        )
        .expect("valid P06 Authority Contract");
        assert_eq!(contract.contract_id().as_str(), AUTHORITY_ID);
        let mut identity = trpg_test_support::identity_service_for_contract(&contract);
        const IDENTITY_PASSWORD: &str = "test authority password long enough";
        identity
            .create_user(
                PLAYER_ID,
                "player-p06-api@example.test",
                IDENTITY_PASSWORD,
                GlobalRole::User,
            )
            .expect("create P06 API player identity");
        let registrar_session = identity
            .login(
                "test-authority-registrar@example.test",
                IDENTITY_PASSWORD,
                104,
            )
            .expect("authenticate P06 authority registrar");
        let registrar = identity
            .authenticate_session(Some(registrar_session.token.expose()), 105)
            .expect("verify P06 authority registrar");
        identity
            .grant_membership(
                &registrar,
                CAMPAIGN_ID,
                PLAYER_ID,
                CampaignRole::Player,
                106,
            )
            .expect("grant canonical P06 player membership");
        let keeper_session = identity
            .login(&format!("{KEEPER_ID}@example.test"), IDENTITY_PASSWORD, 107)
            .expect("authenticate P06 keeper");
        let keeper = identity
            .authenticate_session(Some(keeper_session.token.expose()), 108)
            .expect("verify P06 keeper session");
        let player_session = identity
            .login("player-p06-api@example.test", IDENTITY_PASSWORD, 107)
            .expect("authenticate P06 player");
        let player = identity
            .authenticate_session(Some(player_session.token.expose()), 108)
            .expect("verify P06 player session");
        let credential = identity
            .issue_workload_credential("workflow_001", WorkloadRole::WorkflowEngine, 1, u64::MAX)
            .expect("issue signed workflow credential");
        let workflow = identity
            .authenticate_workload(&credential, 2)
            .expect("authenticate P06 workflow");
        let openfga_address = env::var("P02_OPENFGA_ADDRESS")
            .expect("P02_OPENFGA_ADDRESS is required for the real P06 policy gate")
            .parse()
            .expect("valid OpenFGA address");
        let openfga_store =
            env::var("P02_OPENFGA_STORE_ID").expect("P02_OPENFGA_STORE_ID is required");
        let openfga_model =
            env::var("P02_OPENFGA_MODEL_ID").expect("P02_OPENFGA_MODEL_ID is required");
        let opa_address = env::var("P02_OPA_ADDRESS")
            .expect("P02_OPA_ADDRESS is required for the real P06 policy gate")
            .parse()
            .expect("valid OPA address");
        let opa_revision = env::var("P02_OPA_REVISION")
            .expect("P02_OPA_REVISION is required for the real P06 policy gate");
        let policy = OpenFgaOpaPolicyAdapter::new(
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
        .expect("compose real OpenFGA/OPA adapter");
        let audit_directory = tempfile::Builder::new()
            .prefix("trpg-p06-api-formal-audit-")
            .tempdir()
            .expect("create private randomized P06 audit directory");
        let audit_path = audit_directory.path().join("formal-audit.jsonl");
        let audit = FormalCommitAudit::open(&audit_path, "p06-api-formal-audit-key", &[0xa5; 32])
            .expect("open tamper-evident P06 audit");
        let authorizer = FormalCommitAuthorizer::new(identity.verifier(), policy, audit.clone());
        Self {
            identity,
            workflow,
            keeper,
            player,
            contract,
            authorizer,
            audit,
            audit_path,
            _audit_directory: audit_directory,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn context(
        &mut self,
        actor_id: &str,
        resource_type: &str,
        resource_id: &str,
        visibility_label: &str,
        visibility_subject: Option<&str>,
        suffix: &str,
    ) -> AuthorizedCoreApiContext {
        let requesting_authentication = match actor_id {
            KEEPER_ID => &self.keeper,
            PLAYER_ID => &self.player,
            _ => panic!("test context requested for an unauthenticated actor"),
        };
        let requesting_actor = self
            .identity
            .command_actor(requesting_authentication, self.contract.campaign_id(), 109)
            .expect("identity-minted requesting actor");
        let resource = ResourceRef::new(CAMPAIGN_ID, resource_type, resource_id)
            .expect("valid policy resource");
        let requesting_context = AuthenticatedCommandContext::new(
            requesting_actor.clone(),
            resource.clone(),
            self.contract.binding().expect("locked authority binding"),
            format!("trace_request_{suffix}"),
            requesting_authentication.authenticated_at_unix_ms(),
            requesting_authentication.expires_at_unix_ms(),
        )
        .expect("bind authenticated requester context");
        let workflow_actor = self
            .identity
            .command_actor(&self.workflow, self.contract.campaign_id(), 2)
            .expect("identity-minted workflow actor");
        let authenticated_context = AuthenticatedCommandContext::new(
            workflow_actor,
            resource,
            self.contract.binding().expect("locked authority binding"),
            format!("trace_policy_{suffix}"),
            self.workflow.authenticated_at_unix_ms(),
            self.workflow.expires_at_unix_ms(),
        )
        .expect("bind workflow decision context");
        let visibility = Visibility::try_from_parts(visibility_label, visibility_subject)
            .expect("valid P06 policy visibility");
        let command = CommandEnvelope::new(
            (),
            CommandMetadata {
                command_id: EntityId::new(format!("policy_command_{suffix}"))
                    .expect("valid policy command id"),
                idempotency_key: format!("policy_idempotency_{suffix}"),
                expected_version: 0,
                authority_mode: AuthorityMode::HumanKp,
                visibility,
                fact_provenance: FactProvenance::new(
                    if requesting_actor.role() == &trpg_shared_kernel::ActorRole::HumanKeeper {
                        ProvenanceKind::HumanKeeperStatement
                    } else {
                        ProvenanceKind::UserStatement
                    },
                    format!("policy_source_{suffix}"),
                    actor_id,
                )
                .expect("valid policy provenance"),
                correlation_id: EntityId::new(format!("policy_correlation_{suffix}"))
                    .expect("valid policy correlation"),
                causation_id: EntityId::new(format!("policy_causation_{suffix}"))
                    .expect("valid policy causation"),
                write_path: FormalWritePath::WorkflowDecision,
                authenticated_context,
            },
        );
        let authorization = self
            .authorizer
            .authorize(&self.workflow, None, &command, "workflow", 2)
            .expect("real OpenFGA/OPA formal-write permit");
        AuthorizedCoreApiContext::from_authenticated_contexts(
            requesting_context,
            command.authenticated_context().clone(),
            &self.contract,
            authorization.canonical_audit().clone(),
        )
        .expect("mint typed core API authorization context")
    }

    fn verify_and_cleanup(self) {
        let records = self
            .audit
            .verify()
            .expect("tamper-evident formal policy audit verifies");
        assert!(!records.is_empty());
        assert!(records.iter().all(|record| {
            record.decision == AuditDecision::Permit
                && record.action == "write_official_state"
                && record.openfga_decision_id != "policy-unavailable"
                && record.opa_decision_id != "policy-unavailable"
        }));
        std::fs::remove_file(&self.audit_path).expect("remove session-local policy audit");
    }
}

#[derive(Clone)]
struct RepositoryCampaignCharacterPort {
    repository: CoreDomainRepository,
}

impl RepositoryCampaignCharacterPort {
    #[allow(clippy::too_many_arguments)]
    fn metadata(
        context: &AuthorizedCoreApiContext,
        command: &ApiCommandFields,
        _stream_id: &str,
        _resource_type: &str,
        _action: &str,
        visibility_label: &str,
        visibility_subject: &str,
    ) -> CoreCommandMetadata {
        let provenance_kind = if context.actor_role() == "human_keeper" {
            "human_keeper_statement"
        } else {
            "user_statement"
        };
        CoreCommandMetadata {
            commit_id: format!("commit_{}", command.command_id),
            command_id: command.command_id.clone(),
            idempotency_key: command.idempotency_key.clone(),
            expected_version: command.expected_version,
            requesting_actor_id: context.actor_id().to_owned(),
            requesting_actor_role: context.actor_role().to_owned(),
            authenticated_actor_id: context.workflow_actor_id().to_owned(),
            authenticated_actor_role: context.workflow_actor_role().to_owned(),
            authenticated_actor_origin: EventActorOriginWire::Workload {
                role: "workflow_engine".to_owned(),
            },
            authority_mode: context.authority_mode().to_owned(),
            authority_contract_version: context.authority_contract_version(),
            authority_contract_id: context.authority_contract_id().to_owned(),
            authority_owner: context.authority_owner().to_owned(),
            visibility_label: visibility_label.to_owned(),
            visibility_subject: visibility_subject.to_owned(),
            data_subject_id: if visibility_subject == "not_applicable" {
                "not_applicable".to_owned()
            } else {
                visibility_subject.to_owned()
            },
            provenance_kind: provenance_kind.to_owned(),
            provenance_reference: command.command_id.clone(),
            provenance_recorded_by: context.actor_id().to_owned(),
            correlation_id: command.correlation_id.clone(),
            causation_id: command.causation_id.clone(),
            trace_id: command.trace_id.clone(),
            audit: PolicyAuditDraft {
                actor_id: context.policy_audit().actor_id.clone(),
                actor_origin: context.policy_audit().actor_origin.clone(),
                authentication_reference: context.policy_audit().authentication_reference.clone(),
                resource_type: context.policy_audit().resource_type.clone(),
                resource_id: context.policy_audit().resource_id.clone(),
                action: context.policy_audit().action.clone(),
                requested_role: context.policy_audit().requested_role.clone(),
                openfga_decision_id: context.policy_audit().openfga_decision_id.clone(),
                openfga_policy_revision: context.policy_audit().openfga_policy_revision.clone(),
                opa_decision_id: context.policy_audit().opa_decision_id.clone(),
                opa_policy_revision: context.policy_audit().opa_policy_revision.clone(),
            },
        }
    }

    fn receipt(
        persisted: trpg_data_eventing::event_store_sqlx_outbox_projection::PersistedCommit,
    ) -> CoreApiCommitReceipt {
        CoreApiCommitReceipt {
            last_event_sequence: persisted.last_event_sequence,
            aggregate_version: persisted.last_stream_version,
        }
    }

    fn map_error(error: CoreDomainRepositoryError) -> CoreApiError {
        match error {
            CoreDomainRepositoryError::Forbidden
            | CoreDomainRepositoryError::PolicyEvidenceMismatch => CoreApiError::Forbidden,
            CoreDomainRepositoryError::InvalidInput(field) => CoreApiError::InvalidInput(field),
            CoreDomainRepositoryError::Domain(_) => CoreApiError::Conflict("domain_transition"),
            CoreDomainRepositoryError::ConcurrentStart => {
                CoreApiError::Conflict("concurrent_start")
            }
            CoreDomainRepositoryError::Integrity(_) => CoreApiError::Conflict("integrity_conflict"),
            CoreDomainRepositoryError::NotFound(_) => CoreApiError::Conflict("not_found"),
            CoreDomainRepositoryError::Canonical(_)
            | CoreDomainRepositoryError::Database(_)
            | CoreDomainRepositoryError::Serialization => CoreApiError::Unavailable("repository"),
        }
    }
}

impl CampaignCharacterCommandPort for RepositoryCampaignCharacterPort {
    fn create_campaign<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a CreateCampaignApiRequest,
    ) -> CoreApiFuture<'a, CoreApiCommitReceipt> {
        Box::pin(async move {
            let persisted = self
                .repository
                .create_campaign(
                    &Self::metadata(
                        context,
                        &request.command,
                        &request.campaign_id,
                        "campaign",
                        "campaign.create",
                        "party_visible",
                        "not_applicable",
                    ),
                    &CreateCampaignRequest {
                        campaign_id: request.campaign_id.clone(),
                        owner_user_id: request.owner_user_id.clone(),
                        title: request.title.clone(),
                        room_id: request.room_id.clone(),
                        room_name: request.room_name.clone(),
                        created_at_unix_ms: request.created_at_unix_ms,
                        authority: AuthorityContractSnapshot {
                            contract_id: request.authority.contract_id.clone(),
                            authority_mode: request.authority.authority_mode.clone(),
                            authority_owner: request.authority.authority_owner.clone(),
                            ruleset_version: request.authority.ruleset_version.clone(),
                            house_rules_version: request.authority.house_rules_version.clone(),
                            scenario_version: request.authority.scenario_version.clone(),
                            prompt_version: request.authority.prompt_version.clone(),
                            agent_pack_version: request.authority.agent_pack_version.clone(),
                            tool_schema_version: request.authority.tool_schema_version.clone(),
                            safety_profile_version: request
                                .authority
                                .safety_profile_version
                                .clone(),
                            ai_provider_snapshot: request.authority.ai_provider_snapshot.clone(),
                            model_route_snapshot: request.authority.model_route_snapshot.clone(),
                            character_sheet_template_version: request
                                .authority
                                .character_sheet_template_version
                                .clone(),
                        },
                    },
                )
                .await
                .map_err(Self::map_error)?;
            Ok(Self::receipt(persisted))
        })
    }

    fn issue_invite<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a IssueInviteApiRequest,
    ) -> CoreApiFuture<'a, IssuedInviteApiResponse> {
        Box::pin(async move {
            let role = match request.role.as_str() {
                "PLAYER" => MembershipRole::Player,
                "SPECTATOR" => MembershipRole::Spectator,
                _ => return Err(CoreApiError::InvalidInput("invite_role")),
            };
            let issued = self
                .repository
                .issue_invite(
                    &Self::metadata(
                        context,
                        &request.command,
                        &request.invite_id,
                        "campaign_invite",
                        "campaign.invite.issue",
                        "private_to_player",
                        &request.invited_user_id,
                    ),
                    &IssueInviteRequest {
                        invite_id: request.invite_id.clone(),
                        campaign_id: request.campaign_id.clone(),
                        invited_user_id: request.invited_user_id.clone(),
                        role,
                        expires_at_unix_ms: request.expires_at_unix_ms,
                        now_unix_ms: request.now_unix_ms,
                    },
                )
                .await
                .map_err(Self::map_error)?;
            Ok(IssuedInviteApiResponse {
                invite_id: issued.invite_id,
                raw_token: issued.raw_token,
                expires_at_unix_ms: issued.expires_at_unix_ms,
                receipt: Self::receipt(issued.persisted),
            })
        })
    }

    fn accept_invite<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a AcceptInviteApiRequest,
    ) -> CoreApiFuture<'a, CoreApiCommitReceipt> {
        Box::pin(async move {
            let persisted = self
                .repository
                .accept_invite(
                    &Self::metadata(
                        context,
                        &request.command,
                        &request.invite_id,
                        "campaign_invite",
                        "campaign.invite.accept",
                        "private_to_player",
                        &request.accepting_user_id,
                    ),
                    &AcceptInviteRequest {
                        campaign_id: request.campaign_id.clone(),
                        invite_id: request.invite_id.clone(),
                        accepting_user_id: request.accepting_user_id.clone(),
                        raw_token: request.raw_token.clone(),
                        accepted_at_unix_ms: request.accepted_at_unix_ms,
                    },
                )
                .await
                .map_err(Self::map_error)?;
            Ok(Self::receipt(persisted))
        })
    }

    fn create_character<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a CreateCharacterApiRequest,
    ) -> CoreApiFuture<'a, CoreApiCommitReceipt> {
        Box::pin(async move {
            let sheet: Coc7CharacterSheet = serde_json::from_str(&request.sheet_json)
                .map_err(|_| CoreApiError::InvalidInput("coc7_character_sheet"))?;
            sheet
                .validate()
                .map_err(|_| CoreApiError::InvalidInput("coc7_character_sheet"))?;
            let persisted = self
                .repository
                .create_character(
                    &Self::metadata(
                        context,
                        &request.command,
                        &request.character_id,
                        "character",
                        "character.create",
                        "private_to_player",
                        &request.owner_user_id,
                    ),
                    &CreateCharacterRequest {
                        character_id: request.character_id.clone(),
                        campaign_id: request.campaign_id.clone(),
                        owner_user_id: request.owner_user_id.clone(),
                        display_name: request.display_name.clone(),
                        sheet_version_id: request.sheet_version_id.clone(),
                        sheet_json: request.sheet_json.clone(),
                    },
                )
                .await
                .map_err(Self::map_error)?;
            Ok(Self::receipt(persisted))
        })
    }

    fn submit_character<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a CharacterTransitionApiRequest,
    ) -> CoreApiFuture<'a, CoreApiCommitReceipt> {
        Box::pin(async move {
            let persisted = self
                .repository
                .submit_character(
                    &Self::metadata(
                        context,
                        &request.command,
                        &request.character_id,
                        "character",
                        "character.submit",
                        "private_to_player",
                        context.actor_id(),
                    ),
                    &request.campaign_id,
                    &request.character_id,
                )
                .await
                .map_err(Self::map_error)?;
            Ok(Self::receipt(persisted))
        })
    }

    fn review_character<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a CharacterTransitionApiRequest,
    ) -> CoreApiFuture<'a, CoreApiCommitReceipt> {
        Box::pin(async move {
            let owner_user_id: String = sqlx::query_scalar(
                "SELECT owner_user_id FROM public.characters WHERE character_id = $1",
            )
            .bind(&request.character_id)
            .fetch_one(&self.repository.primary_pool())
            .await
            .map_err(|_| CoreApiError::Conflict("character_not_found"))?;
            let persisted = self
                .repository
                .approve_character_initial_version(
                    &Self::metadata(
                        context,
                        &request.command,
                        &request.character_id,
                        "character",
                        "character.review_initial",
                        "private_to_player",
                        &owner_user_id,
                    ),
                    &request.campaign_id,
                    &request.character_id,
                )
                .await
                .map_err(Self::map_error)?;
            Ok(Self::receipt(persisted))
        })
    }
}

fn valid_sheet_json() -> String {
    serde_json::to_string(&Coc7CharacterSheet {
        name: "Evelyn Hart".to_owned(),
        age: 31,
        occupation: "Investigative journalist".to_owned(),
        era: "1920s".to_owned(),
        birthplace: "Brisbane".to_owned(),
        characteristics: Coc7Characteristics {
            strength: 50,
            dexterity: 60,
            power: 65,
            constitution: 55,
            size: 50,
            appearance: 55,
            intelligence: 70,
            education: 75,
            luck: 60,
        },
        skills: BTreeMap::from([
            ("Library Use".to_owned(), 70),
            ("Psychology".to_owned(), 55),
        ]),
        backstory_anchors: vec![
            "Protects confidential sources".to_owned(),
            "Distrusts official explanations".to_owned(),
        ],
    })
    .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn campaign_invite_and_character_api_use_the_real_repository() {
    let primary_url =
        env::var("P06_DATABASE_URL").expect("P06_DATABASE_URL is required for the real DB gate");
    let witness_url = env::var("P06_WITNESS_DATABASE_URL")
        .expect("P06_WITNESS_DATABASE_URL is required for the independent witness gate");
    let primary_database = env::var("P06_RESET_DATABASE").unwrap();
    let witness_database = env::var("P06_WITNESS_RESET_DATABASE").unwrap();
    let primary = reset_database(&primary_url, &primary_database, false).await;
    let witness = reset_database(&witness_url, &witness_database, true).await;
    witness.close().await;
    let store = PostgresCanonicalStore::connect(
        &primary_url,
        &witness_url,
        "p06-api-integrity-key",
        INTEGRITY_KEY,
        "p06-api-payload-key",
        PAYLOAD_KEY,
    )
    .await
    .expect("connect canonical Event Store and independent witness");
    store
        .prepare_for_service()
        .await
        .expect("migrate P06 API database");
    let repository = CoreDomainRepository::new(primary.clone(), store);
    for (user_id, login) in [(KEEPER_ID, "keeper-p06-api"), (PLAYER_ID, "player-p06-api")] {
        sqlx::query(
            r#"
            INSERT INTO public.users (
                user_id, login_normalized, password_hash, global_role
            ) VALUES ($1, $2, 'not-used-by-api-test', 'USER')
            "#,
        )
        .bind(user_id)
        .bind(login)
        .execute(&primary)
        .await
        .unwrap();
    }
    let api = CampaignCharacterApi::new(Arc::new(RepositoryCampaignCharacterPort { repository }));
    let mut decisions = RealFormalDecisionIssuer::new();
    let keeper = decisions.context(
        KEEPER_ID,
        "campaign",
        CAMPAIGN_ID,
        "party_visible",
        None,
        "keeper_create",
    );
    let campaign_request = CreateCampaignApiRequest {
        command: command("api_campaign_create", 0),
        campaign_id: CAMPAIGN_ID.to_owned(),
        owner_user_id: KEEPER_ID.to_owned(),
        title: "P06 API Campaign".to_owned(),
        room_id: "room_p06_api".to_owned(),
        room_name: "API table".to_owned(),
        created_at_unix_ms: NOW_MS,
        authority: AuthoritySnapshotApiRequest {
            contract_id: AUTHORITY_ID.to_owned(),
            authority_mode: "HUMAN_KP".to_owned(),
            authority_owner: KEEPER_ID.to_owned(),
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
    };
    let campaign_receipt = api
        .create_campaign(&keeper, &campaign_request)
        .await
        .expect("API creates campaign and locked authority");
    assert_eq!(campaign_receipt.aggregate_version, 1);

    let player_before_membership = decisions.context(
        PLAYER_ID,
        "campaign",
        CAMPAIGN_ID,
        "party_visible",
        None,
        "player_denied_create",
    );
    let mut forbidden_campaign = campaign_request.clone();
    forbidden_campaign.owner_user_id = PLAYER_ID.to_owned();
    assert!(matches!(
        api.create_campaign(&player_before_membership, &forbidden_campaign)
            .await,
        Err(CoreApiError::Forbidden)
    ));
    let mut mismatched_campaign = campaign_request.clone();
    mismatched_campaign.campaign_id = "campaign_p06_api_mismatched".to_owned();
    assert!(matches!(
        api.create_campaign(&keeper, &mismatched_campaign).await,
        Err(CoreApiError::InvalidAuthorizationContext)
    ));

    let invite_id = "invite_p06_api_player";
    let invite_issue_context = decisions.context(
        KEEPER_ID,
        "campaign_invite",
        invite_id,
        "private_to_player",
        Some(PLAYER_ID),
        "invite_issue",
    );
    let issued = api
        .issue_invite(
            &invite_issue_context,
            &IssueInviteApiRequest {
                command: command("api_invite_issue", 0),
                campaign_id: CAMPAIGN_ID.to_owned(),
                invite_id: invite_id.to_owned(),
                invited_user_id: PLAYER_ID.to_owned(),
                role: "PLAYER".to_owned(),
                expires_at_unix_ms: NOW_MS + 60_000,
                now_unix_ms: NOW_MS,
            },
        )
        .await
        .expect("API issues invitation");
    let expired_invite_context = decisions.context(
        PLAYER_ID,
        "campaign_invite",
        invite_id,
        "private_to_player",
        Some(PLAYER_ID),
        "player_expired_accept",
    );
    assert!(api
        .accept_invite(
            &expired_invite_context,
            &AcceptInviteApiRequest {
                command: command("api_invite_expired", 1),
                campaign_id: CAMPAIGN_ID.to_owned(),
                invite_id: issued.invite_id.clone(),
                accepting_user_id: PLAYER_ID.to_owned(),
                raw_token: issued.raw_token.clone(),
                accepted_at_unix_ms: NOW_MS + 60_000,
            },
        )
        .await
        .is_err());
    let valid_invite_context = decisions.context(
        PLAYER_ID,
        "campaign_invite",
        invite_id,
        "private_to_player",
        Some(PLAYER_ID),
        "player_valid_accept",
    );
    api.accept_invite(
        &valid_invite_context,
        &AcceptInviteApiRequest {
            command: command("api_invite_accept", 1),
            campaign_id: CAMPAIGN_ID.to_owned(),
            invite_id: issued.invite_id,
            accepting_user_id: PLAYER_ID.to_owned(),
            raw_token: issued.raw_token,
            accepted_at_unix_ms: NOW_MS + 1_000,
        },
    )
    .await
    .expect("API accepts valid invite into real membership");

    let invalid_character_context = decisions.context(
        PLAYER_ID,
        "character",
        "character_p06_api_invalid",
        "private_to_player",
        Some(PLAYER_ID),
        "invalid_character",
    );
    let invalid_sheet_events_before: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM public.event_store WHERE event_type = 'CharacterCreated'",
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    let invalid_sheet = api
        .create_character(
            &invalid_character_context,
            &CreateCharacterApiRequest {
                command: command("api_character_invalid", 0),
                campaign_id: CAMPAIGN_ID.to_owned(),
                character_id: "character_p06_api_invalid".to_owned(),
                owner_user_id: PLAYER_ID.to_owned(),
                display_name: "Invalid".to_owned(),
                sheet_version_id: "sheet_p06_api_invalid_v1".to_owned(),
                sheet_json: r#"{"name":"Invalid","age":12}"#.to_owned(),
            },
        )
        .await;
    assert!(matches!(
        invalid_sheet,
        Err(CoreApiError::InvalidInput("coc7_character_sheet"))
    ));
    let invalid_sheet_events_after: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM public.event_store WHERE event_type = 'CharacterCreated'",
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(invalid_sheet_events_after, invalid_sheet_events_before);

    let character_id = "character_p06_api_player";
    let create_character_context = decisions.context(
        PLAYER_ID,
        "character",
        character_id,
        "private_to_player",
        Some(PLAYER_ID),
        "character_create",
    );
    api.create_character(
        &create_character_context,
        &CreateCharacterApiRequest {
            command: command("api_character_create", 0),
            campaign_id: CAMPAIGN_ID.to_owned(),
            character_id: character_id.to_owned(),
            owner_user_id: PLAYER_ID.to_owned(),
            display_name: "Evelyn Hart".to_owned(),
            sheet_version_id: "sheet_p06_api_player_v1".to_owned(),
            sheet_json: valid_sheet_json(),
        },
    )
    .await
    .expect("API validates and persists COC7 character");
    let submit_character_context = decisions.context(
        PLAYER_ID,
        "character",
        character_id,
        "private_to_player",
        Some(PLAYER_ID),
        "character_submit",
    );
    api.submit_character(
        &submit_character_context,
        &CharacterTransitionApiRequest {
            command: command("api_character_submit", 1),
            campaign_id: CAMPAIGN_ID.to_owned(),
            character_id: character_id.to_owned(),
        },
    )
    .await
    .expect("API submits character");
    let player_review_context = decisions.context(
        PLAYER_ID,
        "character",
        character_id,
        "private_to_player",
        Some(PLAYER_ID),
        "player_review",
    );
    assert!(matches!(
        api.review_character(
            &player_review_context,
            &CharacterTransitionApiRequest {
                command: command("api_character_player_review", 2),
                campaign_id: CAMPAIGN_ID.to_owned(),
                character_id: character_id.to_owned(),
            },
        )
        .await,
        Err(CoreApiError::Forbidden)
    ));
    let keeper_review_context = decisions.context(
        KEEPER_ID,
        "character",
        character_id,
        "private_to_player",
        Some(PLAYER_ID),
        "keeper_review",
    );
    api.review_character(
        &keeper_review_context,
        &CharacterTransitionApiRequest {
            command: command("api_character_review", 2),
            campaign_id: CAMPAIGN_ID.to_owned(),
            character_id: character_id.to_owned(),
        },
    )
    .await
    .expect("keeper reviews and locks initial character version");

    let stored = sqlx::query(
        r#"
        SELECT character.state, character.initial_version_locked,
               sheet.locked AS sheet_locked,
               event.visibility_label,
               event.visibility_subject,
               event.payload_json ? 'protected_payload' AS encrypted
          FROM public.characters AS character
          JOIN public.character_sheet_versions AS sheet
            ON sheet.character_id = character.character_id
          JOIN public.event_store AS event
            ON event.sequence = character.last_event_sequence
         WHERE character.character_id = 'character_p06_api_player'
        "#,
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(stored.get::<String, _>("state"), "APPROVED");
    assert!(stored.get::<bool, _>("initial_version_locked"));
    assert!(stored.get::<bool, _>("sheet_locked"));
    assert_eq!(
        stored.get::<String, _>("visibility_label"),
        "private_to_player"
    );
    assert_eq!(stored.get::<String, _>("visibility_subject"), PLAYER_ID);
    assert!(stored.get::<bool, _>("encrypted"));
    decisions.verify_and_cleanup();
}
