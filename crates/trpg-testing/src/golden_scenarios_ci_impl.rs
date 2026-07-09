use crate::{
    evaluate_testing_quality, standard_contract, TestingQualityAction, TestingQualityCommand,
    TestingQualityEventEnvelope, TestingQualityModuleContract, TestingQualityRepository,
};
use trpg_shared_kernel::{CommandEnvelope, KernelResult};

pub const PROMPT_ID: &str = "CODEX-0874-10-TESTING-QUALITY-95e0ac6e0d";
pub const MODULE: &str = "testing_quality::golden_scenarios_ci_impl";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GoldenScenarioCheck {
    pub name: &'static str,
    pub fixture: &'static str,
    pub assertion: &'static str,
}

pub fn contract() -> TestingQualityModuleContract {
    standard_contract(
        PROMPT_ID,
        MODULE,
        "crates/trpg-testing/src/golden_scenarios_ci_impl.rs",
        "crates/trpg-testing/tests/golden_scenarios_ci_impl_contract_tests.rs",
        TestingQualityAction::VerifyGoldenScenariosCiImpl,
        &[
            "fixtures/scenarios/tutorial_mist_archive.scenario.yaml.md",
            "fixtures/scenarios/golden_salt_bell.scenario.yaml.md",
            "fixtures/actions/golden_salt_bell_action_sequence.v1.json.md",
            "fixtures/stages/detailed/S11_golden_visibility_export_diff_expected.current.json.md",
        ],
        &[
            "tutorial_and_golden_scenarios_are_covered",
            "replay_and_export_diff_are_checked",
            "visibility_leakage_is_blocked",
        ],
    )
}

pub fn golden_checks() -> Vec<GoldenScenarioCheck> {
    vec![
        GoldenScenarioCheck {
            name: "tutorial_mist_archive",
            fixture: "fixtures/scenarios/tutorial_mist_archive.scenario.yaml.md",
            assertion: "complete_playable_loop",
        },
        GoldenScenarioCheck {
            name: "golden_salt_bell",
            fixture: "fixtures/scenarios/golden_salt_bell.scenario.yaml.md",
            assertion: "formal_decisions_replay",
        },
        GoldenScenarioCheck {
            name: "golden_action_sequence",
            fixture: "fixtures/actions/golden_salt_bell_action_sequence.v1.json.md",
            assertion: "server_dice_and_history",
        },
        GoldenScenarioCheck {
            name: "visibility_export_diff",
            fixture: "fixtures/stages/detailed/S11_golden_visibility_export_diff_expected.current.json.md",
            assertion: "redacted_export_matches_expected",
        },
    ]
}

pub fn covers_fixture(fixture: &str) -> bool {
    golden_checks().iter().any(|check| check.fixture == fixture)
}

pub fn evaluate(
    repository: &mut TestingQualityRepository,
    command: &CommandEnvelope<TestingQualityCommand>,
) -> KernelResult<TestingQualityEventEnvelope> {
    evaluate_testing_quality(MODULE, repository, command)
}
