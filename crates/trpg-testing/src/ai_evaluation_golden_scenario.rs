use crate::{
    evaluate_testing_quality, standard_contract, TestingQualityAction, TestingQualityCommand,
    TestingQualityEventEnvelope, TestingQualityModuleContract, TestingQualityRepository,
};
use trpg_shared_kernel::{CommandEnvelope, KernelResult};

pub const PROMPT_ID: &str = "CODEX-0866-10-TESTING-QUALITY-897bc79dc9";
pub const MODULE: &str = "testing_quality::ai_evaluation_golden_scenario";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AiEvaluationGuard {
    pub area: &'static str,
    pub required_path: &'static str,
}

pub fn contract() -> TestingQualityModuleContract {
    standard_contract(
        PROMPT_ID,
        MODULE,
        "crates/trpg-testing/src/ai_evaluation_golden_scenario.rs",
        "crates/trpg-testing/tests/ai_evaluation_golden_scenario_contract_tests.rs",
        TestingQualityAction::VerifyAiEvaluationGoldenScenario,
        &[
            "fixtures/scenarios/golden_salt_bell.scenario.yaml.md",
            "fixtures/actions/golden_salt_bell_action_sequence.v1.json.md",
            "test-data/provider_model_certification_cases.md",
        ],
        &[
            "ai_output_is_tool_request_only",
            "tool_decision_is_event_recorded",
            "visibility_label_is_preserved",
            "model_level_four_is_required_for_ai_keeper",
        ],
    )
}

pub fn evaluation_guards() -> Vec<AiEvaluationGuard> {
    vec![
        AiEvaluationGuard {
            area: "agent_gateway",
            required_path: "Agent Gateway -> Agent Orchestrator/Runtime -> Model Provider Adapter",
        },
        AiEvaluationGuard {
            area: "tool_gate",
            required_path: "tool request -> tool decision -> event store",
        },
        AiEvaluationGuard {
            area: "visibility",
            required_path: "agent context -> tool result -> export redaction",
        },
        AiEvaluationGuard {
            area: "model_certification",
            required_path: "local model level four -> ai keeper orchestrator",
        },
    ]
}

pub fn requires_tool_request_only() -> bool {
    evaluation_guards()
        .iter()
        .any(|guard| guard.area == "tool_gate")
}

pub fn evaluate(
    repository: &mut TestingQualityRepository,
    command: &CommandEnvelope<TestingQualityCommand>,
) -> KernelResult<TestingQualityEventEnvelope> {
    evaluate_testing_quality(MODULE, repository, command)
}
