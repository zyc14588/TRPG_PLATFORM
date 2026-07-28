use std::collections::BTreeMap;
use std::env;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
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
    AcceptInviteRequest, AuthorityContractSnapshot, CoreCommandMetadata, CoreDomainClock,
    CoreDomainRepository, CoreDomainRepositoryError, CreateCampaignRequest, CreateCharacterRequest,
    IssueInviteRequest,
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

#[derive(Debug)]
struct TestClock(AtomicU64);

impl CoreDomainClock for TestClock {
    fn now_unix_ms(&self) -> Result<u64, CoreDomainRepositoryError> {
        Ok(self.0.load(Ordering::SeqCst))
    }
}

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
