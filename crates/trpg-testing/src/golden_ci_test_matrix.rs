use crate::{
    evaluate_testing_quality, standard_contract, TestingQualityAction, TestingQualityCommand,
    TestingQualityEventEnvelope, TestingQualityModuleContract, TestingQualityRepository,
};
use trpg_shared_kernel::{CommandEnvelope, KernelResult};

pub const PROMPT_ID: &str = "CODEX-0858-10-TESTING-QUALITY-dae7b4dc49";
pub const MODULE: &str = "testing_quality::golden_ci_test_matrix";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GoldenCiGate {
    pub name: &'static str,
    pub command: &'static str,
    pub required_assertions: &'static [&'static str],
}

pub fn contract() -> TestingQualityModuleContract {
    standard_contract(
        PROMPT_ID,
        MODULE,
        "crates/trpg-testing/src/golden_ci_test_matrix.rs",
        "crates/trpg-testing/tests/golden_ci_test_matrix_contract_tests.rs",
        TestingQualityAction::VerifyGoldenCiTestMatrix,
        &[
            "stages/s11-testing-quality-golden-ci/TEST_PLAN.md",
            "fixtures/stages/S11_stage_acceptance_fixture.v1.json.md",
            "fixtures/stages/detailed/S11_golden_visibility_export_diff_expected.current.json.md",
            "V1_ACCEPTANCE_EVIDENCE_MATRIX.md",
        ],
        &[
            "golden_scenario_ci_is_required",
            "visibility_leakage_ci_is_required",
            "model_certification_ci_is_required",
            "export_diff_ci_is_required",
        ],
    )
}

pub fn required_gates() -> Vec<GoldenCiGate> {
    vec![
        GoldenCiGate {
            name: "golden_scenarios_ci",
            command: "cargo test -p trpg-testing --test golden_scenarios_ci --all-features",
            required_assertions: &["event_store_replay", "server_dice", "scenario_export"],
        },
        GoldenCiGate {
            name: "visibility_leakage",
            command: "cargo test -p trpg-testing --test visibility_leakage --all-features",
            required_assertions: &["private_redaction", "keeper_only_redaction", "export_diff"],
        },
        GoldenCiGate {
            name: "model_certification_tests",
            command: "cargo test -p trpg-testing --test model_certification_tests --all-features",
            required_assertions: &["level_four_allows_ai_keeper", "lower_levels_are_blocked"],
        },
        GoldenCiGate {
            name: "trpg_testing_contracts",
            command: "cargo test -p trpg-testing --all-features",
            required_assertions: &["command_envelope", "event_store", "fact_provenance"],
        },
    ]
}

pub fn covers_gate(name: &str) -> bool {
    required_gates().iter().any(|gate| gate.name == name)
}

pub fn evaluate(
    repository: &mut TestingQualityRepository,
    command: &CommandEnvelope<TestingQualityCommand>,
) -> KernelResult<TestingQualityEventEnvelope> {
    evaluate_testing_quality(MODULE, repository, command)
}
