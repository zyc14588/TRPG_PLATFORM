
use std::sync::Arc;

use trpg_agent_runtime::adr_0009_agent_governance_agent_governance;
use trpg_agent_runtime::agent_context_assembler;
use trpg_agent_runtime::agent_evaluation_golden_scenario;
use trpg_agent_runtime::agent_runtime::{
    self, AgentDecision, AgentDecisionCommitter, AgentEventPayload, AgentKind, AgentTool,
    ToolRequest,
};
use trpg_agent_runtime::agent_runtime_tool_protocol;
use trpg_agent_runtime::ai_evaluation_golden_scenario;
use trpg_agent_runtime::ai_evaluation_runtime;
use trpg_agent_runtime::local_model_certification::{
    certify_local_model, ensure_ai_keeper_model, CertificationInput, LocalModelLevel,
};
use trpg_agent_runtime::memory_rag;
use trpg_agent_runtime::memory_rag_rag_snapshot;
use trpg_agent_runtime::model_provider::{
    evaluate_cloud_fallback, provider_boundary_snapshot, validate_provider_config, Environment,
    FallbackDecision, ModelRouteSnapshot, ProviderConfig, ProviderType, SecretReference,
};
use trpg_agent_runtime::model_provider_local_cloud;
use trpg_agent_runtime::rag_snapshot::{query_visible_chunks, require_visible_chunk, RagChunk};
use trpg_agent_runtime::tool_protocol;
use trpg_agent_runtime::working_memory_long_memory_rag;
use trpg_agent_runtime::working_memory_rag_rag_snapshot;
use trpg_agent_runtime::{
    ActorRole, AgentToolExecutionOutput, AgentToolExecutor, AuthorityMode, CommandEnvelope,
    EntityId, FormalWritePath, PrincipalScope, Visibility, VisibilityLabel,
};

const RESTRICTED_PLAYER_VISIBLE_TOKENS: &[&str] = &[
    "keeper_truth",
    "secret_operator",
    "npc_true_identity",
    "keeper_only",
    "private_to_player",
    "ai_internal",
];

fn fallback_local_provider() -> ProviderConfig {
    ProviderConfig {
        provider_id: EntityId::new("ollama").unwrap(),
        provider_type: ProviderType::Ollama,
        model_id: "local-model".to_owned(),
        model_artifact_sha256: format!("sha256:{}", "1".repeat(64)),
        base_url: "http://127.0.0.1:11434/v1".to_owned(),
        credential: SecretReference::development("fallback_ollama", 1).unwrap(),
        environment: Environment::Dev,
    }
}

fn fallback_cloud_provider() -> ProviderConfig {
    ProviderConfig {
        provider_id: EntityId::new("cloud").unwrap(),
        provider_type: ProviderType::Cloud,
        model_id: "cloud-model-v1".to_owned(),
        model_artifact_sha256: format!("sha256:{}", "2".repeat(64)),
        base_url: "https://cloud.example.test/v1".to_owned(),
        credential: SecretReference::development("fallback_cloud", 1).unwrap(),
        environment: Environment::Dev,
    }
}

fn fallback_cloud_route() -> ModelRouteSnapshot {
    ModelRouteSnapshot {
        provider_type: ProviderType::Cloud,
        model_id: "cloud-model-v1".to_owned(),
        fallback_policy: "explicit_audited_only",
        privacy_boundary: "explicit_consent_no_silent_fallback",
    }
}

fn ai_kp_command(payload: AgentDecision) -> CommandEnvelope<AgentDecision> {
    trpg_test_support::governed_command(payload, ActorRole::Workflow, AuthorityMode::AiKp)
}

struct SuccessfulToolExecutor;

impl AgentToolExecutor for SuccessfulToolExecutor {
    fn execute(
        &self,
        decision: &AgentDecision,
    ) -> agent_runtime::AgentResult<AgentToolExecutionOutput> {
        Ok(AgentToolExecutionOutput {
            execution_id: format!("execution_{}", decision.decision_id.as_str()),
            result: serde_json::json!({}),
            result_hash: "sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a"
                .to_owned(),
        })
    }
}

fn committer(contract: trpg_agent_runtime::AuthorityContract) -> AgentDecisionCommitter {
    AgentDecisionCommitter::with_tool_executor(
        trpg_test_support::identity_verifier_for_contract(&contract),
        Arc::new(SuccessfulToolExecutor),
    )
    .unwrap()
}

fn assert_no_restricted_player_visible_tokens(text: &str) {
    for token in RESTRICTED_PLAYER_VISIBLE_TOKENS {
        assert!(
            !text.contains(token),
            "{token} leaked in player_visible_text"
        );
    }
}

fn s07_rag_chunks() -> Vec<RagChunk> {
    vec![
        RagChunk::new(
            "rules_coc7_skill_check_001",
            "ruleset_pack",
            Visibility::new(VisibilityLabel::Public),
            "coc7-pack-0.1.0",
            "internal_gameplay",
        )
        .unwrap(),
        RagChunk::new(
            "scenario_keeper_truth_001",
            "scenario",
            Visibility::new(VisibilityLabel::KeeperOnly),
            "golden_salt_bell-0.1.0",
            "campaign_only",
        )
        .unwrap(),
    ]
}

#[test]
fn batch_017_maps_all_prompts_and_primary_modules() {
    let modules = trpg_test_support::normalized_product_modules("trpg-agent-runtime");
    for module in [
        "agent_runtime::agent_context_assembler",
        "agent_runtime::agent_runtime",
        "agent_runtime::ai_evaluation_runtime",
        "agent_runtime::local_model_certification",
        "agent_runtime::memory_rag_rag_snapshot",
        "agent_runtime::model_provider",
        "agent_runtime::tool_protocol",
        "agent_runtime::adr_0009_agent_governance_agent_governance",
        "agent_runtime::agent_runtime_tool_protocol",
        "agent_runtime::agent_evaluation_golden_scenario",
        "agent_runtime::working_memory_long_memory_rag",
        "agent_runtime::rag_snapshot",
        "agent_runtime::model_provider_local_cloud",
        "agent_runtime::ai_evaluation_golden_scenario",
        "agent_runtime::working_memory_rag_rag_snapshot",
        "agent_runtime::memory_rag",
    ] {
        assert!(modules.iter().any(|candidate| candidate == module));
    }

    let primary_wrapper_prompt_ids = [
        trpg_test_support::normalized_prompt_id("trpg-agent-runtime", "agent_context_assembler"),
        trpg_test_support::normalized_prompt_id("trpg-agent-runtime", "ai_evaluation_runtime"),
        trpg_test_support::normalized_prompt_id("trpg-agent-runtime", "local_model_certification"),
        trpg_test_support::normalized_prompt_id("trpg-agent-runtime", "memory_rag_rag_snapshot"),
        trpg_test_support::normalized_prompt_id("trpg-agent-runtime", "model_provider"),
        trpg_test_support::normalized_prompt_id("trpg-agent-runtime", "tool_protocol"),
        trpg_test_support::normalized_prompt_id(
            "trpg-agent-runtime",
            "adr_0009_agent_governance_agent_governance",
        ),
        trpg_test_support::normalized_prompt_id(
            "trpg-agent-runtime",
            "agent_runtime_tool_protocol",
        ),
        trpg_test_support::normalized_prompt_id(
            "trpg-agent-runtime",
            "agent_evaluation_golden_scenario",
        ),
        trpg_test_support::normalized_prompt_id(
            "trpg-agent-runtime",
            "working_memory_long_memory_rag",
        ),
        trpg_test_support::normalized_prompt_id("trpg-agent-runtime", "rag_snapshot"),
        trpg_test_support::normalized_prompt_id("trpg-agent-runtime", "model_provider_local_cloud"),
        trpg_test_support::normalized_prompt_id(
            "trpg-agent-runtime",
            "ai_evaluation_golden_scenario",
        ),
        trpg_test_support::normalized_prompt_id(
            "trpg-agent-runtime",
            "working_memory_rag_rag_snapshot",
        ),
        trpg_test_support::normalized_prompt_id("trpg-agent-runtime", "memory_rag"),
    ];
    for prompt_id in primary_wrapper_prompt_ids {
        trpg_test_support::assert_normalized_prompt_id_exists(&prompt_id);
    }
}

#[test]
fn human_kp_agent_formal_tool_is_draft_only() {
    let request = ToolRequest::formal(AgentKind::KeeperCopilot, AgentTool::ApplySanLoss);
    let decision = agent_runtime::evaluate_agent_tool_request(&AuthorityMode::HumanKp, &request);

    assert!(!decision.tool_executed);
    assert_eq!(decision.downgraded_to, Some(AgentTool::DraftSanLoss));
    assert!(decision.requires_human_confirmation);
    assert!(decision.draft_only);
}

#[test]
fn ai_kp_orchestrator_tool_request_commits_through_event_store() {
    let request = ToolRequest::formal(
        AgentKind::AiKeeperOrchestrator,
        AgentTool::RequestSkillCheck,
    );
    let authentication = trpg_test_support::ai_keeper_authentication("camp_ai_harbor");
    let decision = AgentDecision::new(
        "decision_b017_check",
        request,
        "Spot Hidden check",
        &authentication,
    )
    .unwrap();
    let command = ai_kp_command(decision.clone());
    let contract =
        trpg_test_support::authority_contract("camp_ai_harbor", AuthorityMode::AiKp, 1).unwrap();
    let mut store = common::audited_store(&contract);

    let events = committer(contract)
        .commit(
            &mut store,
            &command,
            &trpg_test_support::workflow_authentication(),
            decision,
            2,
        )
        .unwrap();

    assert_eq!(events.len(), 3);
    assert_eq!(events[0].event_type, "ToolRequestApproved");
    assert_eq!(events[1].event_type, "ToolExecutionSucceeded");
    assert_eq!(events[2].event_type, "DecisionCommitted");
    assert_eq!(store.events().len(), 3);
    match &events[2].payload {
        AgentEventPayload::DecisionCommitted {
            linked_records,
            audit_fields,
            ..
        } => {
            assert!(linked_records.contains(&"DecisionRecord"));
            assert!(linked_records.contains(&"GameEvent"));
            assert!(audit_fields.contains(&"model_provider"));
            assert!(audit_fields.contains(&"visibility_labels"));
        }
        other => panic!("unexpected payload: {other:?}"),
    }
}

#[test]
fn commit_agent_decision_redacts_restricted_fixture_tokens() {
    let dangerous_text =
        "keeper_truth secret_operator npc_true_identity keeper_only private_to_player ai_internal";
    let request = ToolRequest::formal(
        AgentKind::AiKeeperOrchestrator,
        AgentTool::RequestSkillCheck,
    );
    let authentication = trpg_test_support::ai_keeper_authentication("camp_ai_harbor");
    let mut decision = AgentDecision::new(
        "decision_b017_redaction",
        request,
        dangerous_text,
        &authentication,
    )
    .unwrap();
    assert_no_restricted_player_visible_tokens(&decision.player_visible_text);

    decision.player_visible_text = dangerous_text.to_owned();
    let command = ai_kp_command(decision.clone());
    let contract =
        trpg_test_support::authority_contract("camp_ai_harbor", AuthorityMode::AiKp, 1).unwrap();
    let mut store = common::audited_store(&contract);

    let events = committer(contract)
        .commit(
            &mut store,
            &command,
            &trpg_test_support::workflow_authentication(),
            decision,
            2,
        )
        .unwrap();

    match &events[2].payload {
        AgentEventPayload::DecisionCommitted {
            player_visible_text,
            ..
        } => {
            assert_no_restricted_player_visible_tokens(player_visible_text);
            assert!(player_visible_text.contains("[redacted]"));
        }
        other => panic!("unexpected payload: {other:?}"),
    }
}

#[test]
fn expression_agent_cannot_reveal_clue_or_write_directly() {
    let request = ToolRequest::formal(AgentKind::AtmosphereWriter, AgentTool::RevealClue);
    let denied = agent_runtime::evaluate_agent_tool_request(&AuthorityMode::AiKp, &request);
    assert_eq!(denied.error, Some("TOOL_PERMISSION_DENIED"));

    let allowed_request = ToolRequest::formal(
        AgentKind::AiKeeperOrchestrator,
        AgentTool::RequestSkillCheck,
    );
    let authentication = trpg_test_support::ai_keeper_authentication("camp_ai_harbor");
    let decision = AgentDecision::new(
        "decision_direct_write",
        allowed_request,
        "bad write",
        &authentication,
    )
    .unwrap();
    let mut command = ai_kp_command(decision.clone());
    command.write_path = FormalWritePath::DirectAgent;
    let contract =
        trpg_test_support::authority_contract("camp_ai_harbor", AuthorityMode::AiKp, 1).unwrap();
    let mut store = common::audited_store(&contract);

    let error = committer(contract)
        .commit(
            &mut store,
            &command,
            &trpg_test_support::workflow_authentication(),
            decision,
            2,
        )
        .unwrap_err();

    assert_eq!(error.code(), "AGENT_DIRECT_STATE_WRITE_FORBIDDEN");
    assert!(store.events().is_empty());
}
