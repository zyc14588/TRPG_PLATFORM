use crate::{
    evaluate_testing_quality, standard_contract, TestingQualityAction, TestingQualityCommand,
    TestingQualityEventEnvelope, TestingQualityModuleContract, TestingQualityRepository,
};
use trpg_shared_kernel::{CommandEnvelope, KernelResult};

pub const PROMPT_ID: &str = "CODEX-0093-10-TESTING-QUALITY-97f7f731a8";
pub const MODULE: &str = "testing_quality::test_strategy";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestLayer {
    pub name: &'static str,
    pub covers: &'static [&'static str],
}

pub fn contract() -> TestingQualityModuleContract {
    standard_contract(
        PROMPT_ID,
        MODULE,
        "crates/trpg-testing/src/test_strategy.rs",
        "crates/trpg-testing/tests/test_strategy_contract_tests.rs",
        TestingQualityAction::VerifyTestStrategy,
        &[
            "stages/s11-testing-quality-golden-ci/TEST_PLAN.md",
            "stages/s11-testing-quality-golden-ci/TEST_DATA.md",
            "PER_STAGE_FIXTURE_EXPANSION_PLAN.md",
        ],
        &[
            "unit_contract_and_stage_tests_are_layered",
            "negative_cases_are_required",
            "fixtures_are_current_safe",
        ],
    )
}

pub fn required_layers() -> Vec<TestLayer> {
    vec![
        TestLayer {
            name: "module_contract",
            covers: &["command_envelope", "event_store", "visibility_provenance"],
        },
        TestLayer {
            name: "stage_fixture",
            covers: &["golden_scenario", "visibility_leakage", "export_diff"],
        },
        TestLayer {
            name: "negative_cases",
            covers: &[
                "permission_denied",
                "version_conflict",
                "idempotency_conflict",
                "prompt_injection",
            ],
        },
    ]
}

pub fn covers_negative_case(case_id: &str) -> bool {
    required_layers()
        .into_iter()
        .any(|layer| layer.covers.contains(&case_id))
}

pub fn evaluate(
    repository: &mut TestingQualityRepository,
    command: &CommandEnvelope<TestingQualityCommand>,
) -> KernelResult<TestingQualityEventEnvelope> {
    evaluate_testing_quality(MODULE, repository, command)
}
