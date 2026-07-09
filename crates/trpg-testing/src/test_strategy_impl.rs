use crate::{
    evaluate_testing_quality, standard_contract, TestingQualityAction, TestingQualityCommand,
    TestingQualityEventEnvelope, TestingQualityModuleContract, TestingQualityRepository,
};
use trpg_shared_kernel::{CommandEnvelope, KernelResult};

pub const PROMPT_ID: &str = "CODEX-0875-10-TESTING-QUALITY-a2e797e671";
pub const MODULE: &str = "testing_quality::test_strategy_impl";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StageTestCommand {
    pub phase: &'static str,
    pub command: &'static str,
}

pub fn contract() -> TestingQualityModuleContract {
    standard_contract(
        PROMPT_ID,
        MODULE,
        "crates/trpg-testing/src/test_strategy_impl.rs",
        "crates/trpg-testing/tests/test_strategy_impl_contract_tests.rs",
        TestingQualityAction::VerifyTestStrategyImpl,
        &[
            "stages/s11-testing-quality-golden-ci/TEST_PLAN.md",
            "stages/s11-testing-quality-golden-ci/TEST_DATA.md",
            "PER_STAGE_FIXTURE_EXPANSION_PLAN.md",
        ],
        &[
            "minimal_related_check_runs_first",
            "stage_checks_run_after_minimal_check",
            "negative_cases_are_fixture_backed",
        ],
    )
}

pub fn stage_test_commands() -> Vec<StageTestCommand> {
    vec![
        StageTestCommand {
            phase: "minimal_related",
            command: "cargo test -p trpg-testing --all-features",
        },
        StageTestCommand {
            phase: "stage_golden",
            command: "cargo test -p trpg-testing --test golden_scenarios_ci --all-features",
        },
        StageTestCommand {
            phase: "stage_visibility",
            command: "cargo test -p trpg-testing --test visibility_leakage --all-features",
        },
        StageTestCommand {
            phase: "stage_model_certification",
            command: "cargo test -p trpg-testing --test model_certification_tests --all-features",
        },
    ]
}

pub fn command_for_phase(phase: &str) -> Option<&'static str> {
    stage_test_commands()
        .into_iter()
        .find(|entry| entry.phase == phase)
        .map(|entry| entry.command)
}

pub fn evaluate(
    repository: &mut TestingQualityRepository,
    command: &CommandEnvelope<TestingQualityCommand>,
) -> KernelResult<TestingQualityEventEnvelope> {
    evaluate_testing_quality(MODULE, repository, command)
}
