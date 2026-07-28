
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

pub fn denied_formal_commit_policy_endpoints() -> TestPolicyEndpoints {
    static ENDPOINTS: OnceLock<TestPolicyEndpoints> = OnceLock::new();
    *ENDPOINTS.get_or_init(|| TestPolicyEndpoints {
        openfga: spawn_test_policy_server(
            r#"{"allowed":false,"decision_id":"test-openfga-deny"}"#,
            "X-Request-Id: test-openfga-deny\r\n",
        ),
        opa: spawn_test_policy_server(
            r#"{"result":{"allow":false,"decision_id":"test-opa-deny","policy_revision":"test-opa-v1"}}"#,
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
