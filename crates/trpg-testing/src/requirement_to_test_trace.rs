use crate::{
    evaluate_testing_quality, standard_contract, TestingQualityAction, TestingQualityCommand,
    TestingQualityEventEnvelope, TestingQualityModuleContract, TestingQualityRepository,
};
use trpg_shared_kernel::{CommandEnvelope, KernelResult};

pub const PROMPT_ID: &str = "CODEX-0865-10-TESTING-QUALITY-aea366b339";
pub const MODULE: &str = "testing_quality::requirement_to_test_trace";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequirementTestLink {
    pub requirement: &'static str,
    pub test_command: &'static str,
    pub evidence_path: &'static str,
}

pub fn contract() -> TestingQualityModuleContract {
    standard_contract(
        PROMPT_ID,
        MODULE,
        "crates/trpg-testing/src/requirement_to_test_trace.rs",
        "crates/trpg-testing/tests/requirement_to_test_trace_contract_tests.rs",
        TestingQualityAction::VerifyRequirementToTestTrace,
        &[
            "V1_ACCEPTANCE_EVIDENCE_MATRIX.md",
            "stages/s11-testing-quality-golden-ci/TEST_PLAN.md",
            "stages/s11-testing-quality-golden-ci/TEST_DATA.md",
        ],
        &[
            "each_v1_testing_requirement_has_test_command",
            "each_s11_gate_has_fixture",
            "evidence_path_is_declared",
        ],
    )
}

pub fn requirement_links() -> Vec<RequirementTestLink> {
    vec![
        RequirementTestLink {
            requirement: "complete_tutorial_scenario",
            test_command: "cargo test -p trpg-testing --test golden_scenarios_ci --all-features",
            evidence_path: "artifacts/test-reports/golden/golden-salt-bell.md",
        },
        RequirementTestLink {
            requirement: "visibility_leakage_blocked",
            test_command: "cargo test -p trpg-testing --test visibility_leakage --all-features",
            evidence_path: "artifacts/test-reports/visibility/leakage.md",
        },
        RequirementTestLink {
            requirement: "model_certification_enforced",
            test_command:
                "cargo test -p trpg-testing --test model_certification_tests --all-features",
            evidence_path: "artifacts/test-reports/model-certification/cert-levels.md",
        },
        RequirementTestLink {
            requirement: "testing_quality_contracts",
            test_command: "cargo test -p trpg-testing --all-features",
            evidence_path: "evidence/batches/BATCH-039/TEST_RESULTS.md",
        },
    ]
}

pub fn has_test_for(requirement: &str) -> bool {
    requirement_links()
        .iter()
        .any(|link| link.requirement == requirement && !link.test_command.is_empty())
}

pub fn evaluate(
    repository: &mut TestingQualityRepository,
    command: &CommandEnvelope<TestingQualityCommand>,
) -> KernelResult<TestingQualityEventEnvelope> {
    evaluate_testing_quality(MODULE, repository, command)
}
