use crate::{
    evaluate_testing_quality, standard_contract, TestingQualityAction, TestingQualityCommand,
    TestingQualityEventEnvelope, TestingQualityModuleContract, TestingQualityRepository,
};
use trpg_shared_kernel::{CommandEnvelope, KernelResult};

pub const PROMPT_ID: &str = "CODEX-0906-10-TESTING-QUALITY-d70cab3757";
pub const MODULE: &str = "testing_quality::golden_scenarios_ci";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GoldenScenarioCiCheck {
    pub name: &'static str,
    pub fixture: &'static str,
    pub assertion: &'static str,
}

pub fn contract() -> TestingQualityModuleContract {
    standard_contract(
        PROMPT_ID,
        MODULE,
        "crates/trpg-testing/src/golden_scenarios_ci.rs",
        "crates/trpg-testing/tests/golden_scenarios_ci_contract_tests.rs",
        TestingQualityAction::VerifyGoldenScenariosCi,
        &[
            "fixtures/scenarios/golden_salt_bell.scenario.yaml.md",
            "fixtures/actions/golden_salt_bell_action_sequence.v1.json.md",
            "fixtures/stages/detailed/S11_golden_visibility_export_diff_expected.current.json.md",
            "V1_ACCEPTANCE_EVIDENCE_MATRIX.md",
        ],
        &[
            "formal_rulings_use_command_workflow_decision_event_store_projection",
            "server_dice_source_is_required",
            "visibility_export_diff_blocks_hidden_clue_leak",
            "fact_provenance_and_audit_records_are_kept",
        ],
    )
}

pub fn decision_pipeline() -> &'static [&'static str] {
    &[
        "Command",
        "Workflow",
        "Decision",
        "EventStore",
        "Projection",
    ]
}

pub fn required_fixture_markers() -> &'static [&'static str] {
    &[
        "request_skill_check",
        "prompt_injection_detected",
        "dice_source",
        "history_deleted",
        "visibility_leakage",
        "redacted_fields",
    ]
}

pub fn expected_records() -> &'static [&'static str] {
    &["ScenarioTestReport", "ExportDiffReport"]
}

pub fn expected_error_codes() -> &'static [&'static str] {
    &[
        "VISIBILITY_LEAKAGE_DETECTED",
        "GOLDEN_SCENARIO_RULE_VIOLATION",
        "KEEPER_SECRET_REVEALED",
    ]
}

pub fn golden_ci_checks() -> Vec<GoldenScenarioCiCheck> {
    vec![
        GoldenScenarioCiCheck {
            name: "formal_decision_path",
            fixture: "fixtures/actions/golden_salt_bell_action_sequence.v1.json.md",
            assertion: "command_workflow_decision_event_store_projection",
        },
        GoldenScenarioCiCheck {
            name: "server_dice",
            fixture: "fixtures/actions/golden_salt_bell_action_sequence.v1.json.md",
            assertion: "server_dice_source",
        },
        GoldenScenarioCiCheck {
            name: "visibility_export_diff",
            fixture: "fixtures/stages/detailed/S11_golden_visibility_export_diff_expected.current.json.md",
            assertion: "visibility_leakage_detected",
        },
        GoldenScenarioCiCheck {
            name: "audit_record",
            fixture: "fixtures/stages/detailed/S11_golden_visibility_export_diff_expected.current.json.md",
            assertion: "fact_provenance_kept",
        },
    ]
}

pub fn covers_fixture(fixture: &str) -> bool {
    golden_ci_checks()
        .iter()
        .any(|check| check.fixture == fixture)
}

pub fn covers_expected_record(record: &str) -> bool {
    expected_records().contains(&record)
}

pub fn covers_expected_error(error_code: &str) -> bool {
    expected_error_codes().contains(&error_code)
}

pub fn requires_event_store_path() -> bool {
    decision_pipeline()
        == [
            "Command",
            "Workflow",
            "Decision",
            "EventStore",
            "Projection",
        ]
}

pub fn requires_server_dice() -> bool {
    golden_ci_checks()
        .iter()
        .any(|check| check.assertion == "server_dice_source")
}

pub fn requires_visibility_redaction() -> bool {
    golden_ci_checks()
        .iter()
        .any(|check| check.assertion == "visibility_leakage_detected")
}

pub fn evaluate(
    repository: &mut TestingQualityRepository,
    command: &CommandEnvelope<TestingQualityCommand>,
) -> KernelResult<TestingQualityEventEnvelope> {
    evaluate_testing_quality(MODULE, repository, command)
}
