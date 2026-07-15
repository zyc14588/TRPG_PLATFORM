use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;

use trpg_shared_kernel::{
    Actor, ActorRole, AgentClass, AuthenticatedCommandContext, AuthorityContract,
    AuthorityContractDraft, AuthorityMode, AuthorityVersionSnapshotDraft, CanonicalCommitPort,
    CanonicalCommitReceipt, CanonicalCommitRequest, CommandEnvelope, CommandMetadata, EntityId,
    FactProvenance, FormalWritePath, KernelResult, ProvenanceKind, ResourceRef, TrpgError,
    Visibility, VisibilityLabel, WorkloadRole,
};

const TEST_IDENTITY_SIGNING_KEY: [u8; 32] = [0x5a; 32];

#[derive(Debug, Default)]
struct TestCanonicalCommitPort {
    state: Mutex<TestCanonicalState>,
}

#[derive(Debug, Default)]
struct TestCanonicalState {
    stream_versions: HashMap<String, u64>,
    idempotency_keys: HashSet<(String, String)>,
}

impl CanonicalCommitPort for TestCanonicalCommitPort {
    fn commit(&self, request: &CanonicalCommitRequest) -> KernelResult<CanonicalCommitReceipt> {
        if request.events.is_empty() {
            return Err(TrpgError::AuditIntegrityViolation);
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| TrpgError::AuditIntegrityViolation)?;
        let actual_version = state
            .stream_versions
            .get(&request.campaign_id)
            .copied()
            .unwrap_or(0);
        if request.expected_version != actual_version {
            return Err(TrpgError::ExpectedVersionConflict {
                expected: request.expected_version,
                actual: actual_version,
            });
        }
        let idempotency = (request.campaign_id.clone(), request.idempotency_key.clone());
        if state.idempotency_keys.contains(&idempotency) {
            return Err(TrpgError::DuplicateCommand);
        }
        let event_count =
            u64::try_from(request.events.len()).map_err(|_| TrpgError::AuditIntegrityViolation)?;
        let first_stream_version = request
            .expected_version
            .checked_add(1)
            .ok_or(TrpgError::AuditIntegrityViolation)?;
        let last_stream_version = request
            .expected_version
            .checked_add(event_count)
            .ok_or(TrpgError::AuditIntegrityViolation)?;
        state.idempotency_keys.insert(idempotency);
        state
            .stream_versions
            .insert(request.campaign_id.clone(), last_stream_version);
        Ok(CanonicalCommitReceipt {
            first_stream_version,
            last_stream_version,
        })
    }
}

pub fn test_canonical_commit_port() -> Arc<dyn CanonicalCommitPort> {
    Arc::new(TestCanonicalCommitPort::default())
}

const NORMALIZED_PROMPT_MAP: &str =
    include_str!("../../../../docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md");

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NormalizedPromptBinding {
    pub prompt_id: String,
    pub crate_name: String,
    pub rust_module: String,
    pub task_type: String,
    pub output_target: String,
}

pub fn normalized_prompt_bindings() -> Vec<NormalizedPromptBinding> {
    NORMALIZED_PROMPT_MAP
        .lines()
        .filter_map(|line| {
            let cells = line
                .split('|')
                .skip(1)
                .take(6)
                .map(clean_table_cell)
                .collect::<Vec<_>>();
            if cells.len() != 6 || !cells[0].starts_with("CODEX-") {
                return None;
            }
            Some(NormalizedPromptBinding {
                prompt_id: cells[0].clone(),
                crate_name: cells[2].clone(),
                rust_module: cells[3].clone(),
                task_type: cells[4].clone(),
                output_target: cells[5].clone(),
            })
        })
        .collect()
}

pub fn assert_normalized_prompt_binding(crate_name: &str, module_name: &str, prompt_id: &str) {
    let module_suffix = format!("::{module_name}");
    assert!(
        normalized_prompt_bindings().iter().any(|binding| {
            binding.prompt_id == prompt_id
                && binding.crate_name == crate_name
                && binding.rust_module.ends_with(&module_suffix)
                && binding.task_type == "product-code"
        }),
        "missing normalized product-code binding for {crate_name}::{module_name} -> {prompt_id}"
    );
}

pub fn normalized_product_modules(crate_name: &str) -> Vec<String> {
    let mut modules = normalized_prompt_bindings()
        .into_iter()
        .filter(|binding| binding.crate_name == crate_name && binding.task_type == "product-code")
        .map(|binding| binding.rust_module)
        .collect::<Vec<_>>();
    modules.sort();
    modules.dedup();
    modules
}

pub fn normalized_prompt_id(crate_name: &str, module_name: &str) -> String {
    let output_target = format!("crates/{crate_name}/src/{module_name}.rs");
    let matches = normalized_prompt_bindings()
        .into_iter()
        .filter(|binding| {
            binding.crate_name == crate_name
                && binding.task_type == "product-code"
                && binding.output_target == output_target
        })
        .map(|binding| binding.prompt_id)
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "expected one normalized source binding for {output_target}, got {matches:?}"
    );
    matches.into_iter().next().unwrap()
}

pub fn normalized_prompt_ids_for_module(crate_name: &str, module_name: &str) -> Vec<String> {
    let module_suffix = format!("::{module_name}");
    normalized_prompt_bindings()
        .into_iter()
        .filter(|binding| {
            binding.crate_name == crate_name
                && binding.task_type == "product-code"
                && binding.rust_module.ends_with(&module_suffix)
        })
        .map(|binding| binding.prompt_id)
        .collect()
}

pub fn assert_normalized_product_module(crate_name: &str, module_name: &str) {
    drop(normalized_prompt_id(crate_name, module_name));
}

pub fn assert_normalized_prompt_id_exists(prompt_id: &str) {
    assert!(
        normalized_prompt_bindings()
            .iter()
            .any(|binding| binding.prompt_id == prompt_id),
        "missing prompt ID from normalized execution map: {prompt_id}"
    );
}

fn clean_table_cell(cell: &str) -> String {
    cell.trim().trim_matches('`').to_owned()
}

pub fn governed_command<T>(
    payload: T,
    actor_role: ActorRole,
    authority_mode: AuthorityMode,
) -> CommandEnvelope<T> {
    let campaign_id = match authority_mode {
        AuthorityMode::HumanKp => "camp_human_archive",
        AuthorityMode::AiKp => "camp_ai_harbor",
    };
    let contract = authority_contract(campaign_id, authority_mode.clone(), 1)
        .expect("valid fixture authority contract");
    governed_command_for_contract(&contract, payload, actor_role)
}

pub fn governed_command_for_contract<T>(
    contract: &AuthorityContract,
    payload: T,
    actor_role: ActorRole,
) -> CommandEnvelope<T> {
    let actor = actor_for_role(
        actor_role,
        contract.campaign_id().as_str(),
        contract.authority_owner().as_str(),
    );
    let context = AuthenticatedCommandContext::new(
        actor,
        ResourceRef::new(
            contract.campaign_id().as_str(),
            "campaign",
            contract.campaign_id().as_str(),
        )
        .expect("valid fixture resource"),
        contract.binding().expect("valid fixture authority binding"),
        "trace_001",
        1,
        u64::MAX,
    )
    .expect("valid authenticated fixture context");
    CommandEnvelope::new(
        payload,
        CommandMetadata {
            command_id: EntityId::new("command_001").expect("valid fixture command id"),
            idempotency_key: "idem_001".to_owned(),
            expected_version: 0,
            authority_mode: contract.mode().clone(),
            visibility: Visibility::new(VisibilityLabel::SystemOnly),
            fact_provenance: FactProvenance::new(
                ProvenanceKind::RulesEngineDecision,
                "fact_001",
                "rules_001",
            )
            .expect("valid fixture provenance"),
            correlation_id: EntityId::new("corr_001").expect("valid fixture correlation id"),
            causation_id: EntityId::new("cause_001").expect("valid fixture causation id"),
            write_path: FormalWritePath::WorkflowDecision,
            authenticated_context: context,
        },
    )
}

pub fn authority_contract(
    campaign_id: &str,
    mode: AuthorityMode,
    version: u64,
) -> trpg_shared_kernel::KernelResult<AuthorityContract> {
    let owner = match &mode {
        AuthorityMode::HumanKp => "user_human_kp",
        AuthorityMode::AiKp => "ai_kp_local_level4",
    };
    authority_contract_with_owner(campaign_id, mode, owner, version)
}

pub fn authority_contract_with_owner(
    campaign_id: &str,
    mode: AuthorityMode,
    owner: &str,
    version: u64,
) -> trpg_shared_kernel::KernelResult<AuthorityContract> {
    AuthorityContract::new_locked(AuthorityContractDraft {
        contract_id: format!("authority_contract_{campaign_id}_{version}"),
        campaign_id: campaign_id.to_owned(),
        mode,
        authority_owner: owner.to_owned(),
        version,
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
        created_at_unix_ms: 1,
    })
}

pub fn ai_keeper_authentication(campaign_id: &str) -> trpg_identity::AuthenticationContext {
    let identity = trpg_identity::IdentityService::new(&TEST_IDENTITY_SIGNING_KEY, 60_000)
        .expect("valid test identity service");
    let credential = identity
        .issue_agent_run_credential(
            "agent_run_test",
            "ai_kp_local_level4",
            campaign_id,
            trpg_identity::AgentClass::AiKeeperOrchestrator,
            1,
            10_000,
        )
        .expect("valid signed test agent credential");
    identity
        .authenticate_agent_run(&credential, 2)
        .expect("valid test agent authentication")
}

pub fn workflow_authentication() -> trpg_identity::AuthenticationContext {
    let identity = trpg_identity::IdentityService::new(&TEST_IDENTITY_SIGNING_KEY, 60_000)
        .expect("valid test identity service");
    let credential = identity
        .issue_workload_credential(
            "workflow_001",
            trpg_identity::WorkloadRole::WorkflowEngine,
            1,
            10_000,
        )
        .expect("valid signed workflow credential");
    identity
        .authenticate_workload(&credential, 2)
        .expect("valid workflow authentication")
}

pub fn identity_verifier() -> trpg_identity::IdentityVerifier {
    trpg_identity::IdentityService::new(&TEST_IDENTITY_SIGNING_KEY, 60_000)
        .expect("valid test identity service")
        .verifier()
}

pub fn identity_verifier_for_contract(
    contract: &AuthorityContract,
) -> trpg_identity::IdentityVerifier {
    identity_service_for_contract(contract).verifier()
}

pub fn identity_service_for_contract(
    contract: &AuthorityContract,
) -> trpg_identity::IdentityService {
    use trpg_identity::{CampaignRole, GlobalRole, IdentityService};

    const REGISTRAR_ID: &str = "test_authority_registrar";
    const REGISTRAR_LOGIN: &str = "test-authority-registrar@example.test";
    const PASSWORD: &str = "test authority password long enough";

    let mut identity =
        IdentityService::new(&TEST_IDENTITY_SIGNING_KEY, 60_000).expect("valid identity root");
    identity
        .create_user(
            REGISTRAR_ID,
            REGISTRAR_LOGIN,
            PASSWORD,
            GlobalRole::ServerOwner,
        )
        .expect("valid authority registrar");
    if contract.mode() == &AuthorityMode::HumanKp
        && contract.authority_owner().as_str() != REGISTRAR_ID
    {
        let owner_login = format!("{}@example.test", contract.authority_owner().as_str());
        identity
            .create_user(
                contract.authority_owner().as_str(),
                &owner_login,
                PASSWORD,
                GlobalRole::User,
            )
            .expect("valid human authority owner");
    }
    let session = identity
        .login(REGISTRAR_LOGIN, PASSWORD, 100)
        .expect("authority registrar can authenticate");
    let registrar = identity
        .authenticate_session(Some(session.token.expose()), 101)
        .expect("valid authority registrar context");
    if contract.mode() == &AuthorityMode::HumanKp {
        identity
            .grant_membership(
                &registrar,
                contract.campaign_id().as_str(),
                contract.authority_owner().as_str(),
                CampaignRole::HumanKeeper,
                102,
            )
            .expect("canonical human keeper membership");
    }
    identity
        .register_authority_contract(&registrar, contract.clone(), 103)
        .expect("canonical authority registration");
    identity
}

pub fn actor_for_role(role: ActorRole, campaign_id: &str, authority_owner: &str) -> Actor {
    match role {
        ActorRole::HumanKeeper => {
            Actor::authenticated_user(authority_owner, ActorRole::HumanKeeper, "session_human_kp")
        }
        ActorRole::AiKeeper => Actor::verified_agent_run(
            authority_owner,
            "agent_run_001",
            AgentClass::AiKeeperOrchestrator,
            campaign_id,
        ),
        ActorRole::Workflow => {
            Actor::verified_workload("workflow_001", WorkloadRole::WorkflowEngine)
        }
        ActorRole::RulesEngine => Actor::verified_workload("rules_001", WorkloadRole::RulesEngine),
        ActorRole::System => Actor::verified_workload("system_001", WorkloadRole::ApiServer),
        ActorRole::ServerOwner
        | ActorRole::CampaignOwner
        | ActorRole::Investigator
        | ActorRole::Moderator
        | ActorRole::Spectator => Actor::authenticated_user("user_001", role, "session_user_001"),
    }
    .expect("valid fixture actor")
}

#[derive(Clone, Copy, Debug)]
pub struct TestPolicyEndpoints {
    pub openfga: SocketAddr,
    pub opa: SocketAddr,
    pub openfga_model: &'static str,
    pub opa_revision: &'static str,
}

pub fn formal_commit_policy_endpoints() -> TestPolicyEndpoints {
    static ENDPOINTS: OnceLock<TestPolicyEndpoints> = OnceLock::new();
    *ENDPOINTS.get_or_init(|| TestPolicyEndpoints {
        openfga: spawn_test_policy_server(
            r#"{"allowed":true,"decision_id":"test-openfga-permit"}"#,
            "X-Request-Id: test-openfga-permit\r\n",
        ),
        opa: spawn_test_policy_server(
            r#"{"result":{"allow":true,"decision_id":"test-opa-permit","policy_revision":"test-opa-v1"}}"#,
            "",
        ),
        openfga_model: "test-openfga-model-v1",
        opa_revision: "test-opa-v1",
    })
}

pub fn formal_commit_identity_for_contract(
    contract: &AuthorityContract,
) -> (
    trpg_identity::IdentityVerifier,
    trpg_identity::AuthenticationContext,
) {
    let identity = identity_service_for_contract(contract);
    let credential = identity
        .issue_workload_credential(
            "workflow_001",
            trpg_identity::WorkloadRole::WorkflowEngine,
            1,
            u64::MAX,
        )
        .expect("valid long-lived test workflow credential");
    let authentication = identity
        .authenticate_workload(&credential, 2)
        .expect("valid test workflow authentication");
    (identity.verifier(), authentication)
}

pub fn system_replay_authorization(
    contract: &AuthorityContract,
) -> trpg_identity::ReplayAuthorization {
    let identity = identity_service_for_contract(contract);
    let credential = identity
        .issue_workload_credential(
            "realtime_replay_test",
            trpg_identity::WorkloadRole::RealtimeServer,
            1,
            u64::MAX,
        )
        .expect("valid replay workload credential");
    let authentication = identity
        .authenticate_workload(&credential, 2)
        .expect("valid replay workload authentication");
    identity
        .verifier()
        .authorize_replay(&authentication, contract.campaign_id(), 2)
        .expect("campaign-bound replay authorization")
}

pub fn player_replay_authorization(
    contract: &AuthorityContract,
) -> trpg_identity::ReplayAuthorization {
    player_replay_authorization_for(contract, "replay_player")
}

pub fn player_replay_authorization_for(
    contract: &AuthorityContract,
    player_id: &str,
) -> trpg_identity::ReplayAuthorization {
    use trpg_identity::{CampaignRole, GlobalRole};

    const PASSWORD: &str = "test replay password long enough";
    let mut identity = identity_service_for_contract(contract);
    identity
        .create_user(
            "replay_registrar",
            "replay-registrar@example.test",
            PASSWORD,
            GlobalRole::ServerOwner,
        )
        .expect("valid replay registrar");
    identity
        .create_user(
            player_id,
            &format!("{player_id}@example.test"),
            PASSWORD,
            GlobalRole::User,
        )
        .expect("valid replay player");
    let registrar_session = identity
        .login("replay-registrar@example.test", PASSWORD, 200)
        .expect("replay registrar login");
    let registrar = identity
        .authenticate_session(Some(registrar_session.token.expose()), 201)
        .expect("replay registrar authentication");
    identity
        .grant_membership(
            &registrar,
            contract.campaign_id().as_str(),
            player_id,
            CampaignRole::Player,
            202,
        )
        .expect("replay player membership");
    let player_session = identity
        .login(&format!("{player_id}@example.test"), PASSWORD, 203)
        .expect("replay player login");
    let player = identity
        .authenticate_session(Some(player_session.token.expose()), 204)
        .expect("replay player authentication");
    identity
        .verifier()
        .authorize_replay(&player, contract.campaign_id(), 205)
        .expect("player replay authorization")
}

fn spawn_test_policy_server(body: &'static str, extra_headers: &'static str) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test policy server");
    let address = listener.local_addr().expect("test policy server address");
    thread::Builder::new()
        .name("formal-commit-test-policy".to_owned())
        .spawn(move || {
            for connection in listener.incoming() {
                let Ok(mut stream) = connection else {
                    break;
                };
                if read_complete_http_request(&mut stream).is_err() {
                    continue;
                }
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n{extra_headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
            }
        })
        .expect("spawn test policy server");
    address
}

fn read_complete_http_request(stream: &mut TcpStream) -> std::io::Result<()> {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let count = stream.read(&mut buffer)?;
        if count == 0 {
            return Ok(());
        }
        request.extend_from_slice(&buffer[..count]);
        let Some(boundary) = request.windows(4).position(|window| window == b"\r\n\r\n") else {
            continue;
        };
        let headers = String::from_utf8_lossy(&request[..boundary]);
        let content_length = headers
            .lines()
            .filter_map(|line| line.split_once(':'))
            .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
            .and_then(|(_, value)| value.trim().parse::<usize>().ok())
            .unwrap_or(0);
        if request.len() >= boundary + 4 + content_length {
            return Ok(());
        }
    }
}
