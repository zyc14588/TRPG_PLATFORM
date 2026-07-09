use crate::{
    evaluate_testing_quality, standard_contract, TestingQualityAction, TestingQualityCommand,
    TestingQualityEventEnvelope, TestingQualityModuleContract, TestingQualityRepository,
};
use trpg_shared_kernel::{CommandEnvelope, KernelResult};

pub const PROMPT_ID: &str = "CODEX-0845-10-TESTING-QUALITY-6254d78940";
pub const MODULE: &str = "testing_quality::testing_golden_scenarios_ci";

pub fn contract() -> TestingQualityModuleContract {
    standard_contract(
        PROMPT_ID,
        MODULE,
        "crates/trpg-testing/src/testing_golden_scenarios_ci.rs",
        "crates/trpg-testing/tests/testing_golden_scenarios_ci_contract_tests.rs",
        TestingQualityAction::VerifyGoldenScenarioSuite,
        &[
            "fixtures/scenarios/golden_salt_bell.scenario.yaml.md",
            "fixtures/actions/golden_salt_bell_action_sequence.v1.json.md",
            "fixtures/stages/detailed/S11_golden_visibility_export_diff_expected.current.json.md",
        ],
        &[
            "golden_salt_bell_loaded",
            "prompt_injection_is_audited",
            "export_redaction_is_asserted",
            "server_dice_source_is_required",
        ],
    )
}

pub fn required_fixture_markers() -> &'static [&'static str] {
    &[
        "golden_salt_bell",
        "visibility_leakage",
        "secret_roll",
        "export_redaction",
        "prompt_injection_detected",
        "dice_source",
    ]
}

pub fn evaluate(
    repository: &mut TestingQualityRepository,
    command: &CommandEnvelope<TestingQualityCommand>,
) -> KernelResult<TestingQualityEventEnvelope> {
    evaluate_testing_quality(MODULE, repository, command)
}
