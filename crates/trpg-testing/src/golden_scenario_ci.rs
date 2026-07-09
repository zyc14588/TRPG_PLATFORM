use crate::{
    evaluate_testing_quality, standard_contract, TestingQualityAction, TestingQualityCommand,
    TestingQualityEventEnvelope, TestingQualityModuleContract, TestingQualityRepository,
};
use trpg_shared_kernel::{CommandEnvelope, KernelResult};

pub const PROMPT_ID: &str = "CODEX-0846-10-TESTING-QUALITY-4191e3f193";
pub const MODULE: &str = "testing_quality::golden_scenario_ci";

pub fn contract() -> TestingQualityModuleContract {
    standard_contract(
        PROMPT_ID,
        MODULE,
        "crates/trpg-testing/src/golden_scenario_ci.rs",
        "crates/trpg-testing/tests/golden_scenario_ci_contract_tests.rs",
        TestingQualityAction::VerifyGoldenScenarioCi,
        &[
            "fixtures/scenarios/golden_salt_bell.scenario.yaml.md",
            "fixtures/actions/golden_salt_bell_action_sequence.v1.json.md",
        ],
        &[
            "formal_rulings_use_command_workflow_decision_event_projection",
            "server_dice_not_agent_dice",
            "history_is_not_deleted_on_reconsideration",
        ],
    )
}

pub fn official_decision_pipeline() -> &'static [&'static str] {
    &[
        "Command",
        "Workflow",
        "Decision",
        "EventStore",
        "Projection",
    ]
}

pub fn server_dice_required() -> bool {
    true
}

pub fn evaluate(
    repository: &mut TestingQualityRepository,
    command: &CommandEnvelope<TestingQualityCommand>,
) -> KernelResult<TestingQualityEventEnvelope> {
    evaluate_testing_quality(MODULE, repository, command)
}
