use trpg_shared_kernel::{
    Actor, ActorRole, AgentClass, AuthenticatedCommandContext, AuthorityContract,
    AuthorityContractDraft, AuthorityMode, AuthorityVersionSnapshotDraft, CommandEnvelope,
    CommandMetadata, EntityId, FactProvenance, FormalWritePath, ProvenanceKind, ResourceRef,
    Visibility, VisibilityLabel, WorkloadRole,
};

const TEST_IDENTITY_SIGNING_KEY: [u8; 32] = [0x5a; 32];

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

pub fn identity_verifier() -> trpg_identity::IdentityVerifier {
    trpg_identity::IdentityService::new(&TEST_IDENTITY_SIGNING_KEY, 60_000)
        .expect("valid test identity service")
        .verifier()
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
