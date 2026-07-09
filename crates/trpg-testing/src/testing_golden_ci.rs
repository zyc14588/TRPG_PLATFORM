use crate::{
    evaluate_testing_quality, standard_contract, TestingQualityAction, TestingQualityCommand,
    TestingQualityEventEnvelope, TestingQualityModuleContract, TestingQualityRepository,
};
use trpg_shared_kernel::{CommandEnvelope, KernelResult};

pub const PROMPT_ID: &str = "CODEX-0094-10-TESTING-QUALITY-6ac95ec41f";
pub const MODULE: &str = "testing_quality::testing_golden_ci";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GoldenCiGate {
    pub name: &'static str,
    pub command: &'static str,
    pub required_fixture: &'static str,
}

pub fn contract() -> TestingQualityModuleContract {
    standard_contract(
        PROMPT_ID,
        MODULE,
        "crates/trpg-testing/src/testing_golden_ci.rs",
        "crates/trpg-testing/tests/testing_golden_ci_contract_tests.rs",
        TestingQualityAction::VerifyGoldenCi,
        &[
            "fixtures/stages/S11_stage_acceptance_fixture.v1.json.md",
            "fixtures/stages/detailed/S11_golden_visibility_export_diff_expected.current.json.md",
        ],
        &[
            "golden_scenario_passes",
            "no_visibility_leakage",
            "exports_diff_as_expected",
            "eval_report_written",
        ],
    )
}

pub fn required_gates() -> Vec<GoldenCiGate> {
    vec![
        GoldenCiGate {
            name: "crate_contracts",
            command: "cargo test -p trpg-testing --all-features",
            required_fixture: "fixtures/stages/S11_stage_acceptance_fixture.v1.json.md",
        },
        GoldenCiGate {
            name: "golden_scenario",
            command: "cargo test -p trpg-testing --test testing_golden_scenarios_ci_contract_tests",
            required_fixture: "fixtures/scenarios/golden_salt_bell.scenario.yaml.md",
        },
        GoldenCiGate {
            name: "visibility_leakage",
            command: "cargo test -p trpg-testing --test visibility_leakage_tests_contract_tests",
            required_fixture: "test-data/visibility_leakage_cases.md",
        },
        GoldenCiGate {
            name: "model_certification",
            command: "cargo test -p trpg-testing --test model_certification_tests_contract_tests",
            required_fixture: "test-data/provider_model_certification_cases.md",
        },
    ]
}

pub fn evaluate(
    repository: &mut TestingQualityRepository,
    command: &CommandEnvelope<TestingQualityCommand>,
) -> KernelResult<TestingQualityEventEnvelope> {
    evaluate_testing_quality(MODULE, repository, command)
}
