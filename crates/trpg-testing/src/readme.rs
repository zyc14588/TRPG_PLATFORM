use crate::{
    evaluate_testing_quality, standard_contract, TestingQualityAction, TestingQualityCommand,
    TestingQualityEventEnvelope, TestingQualityModuleContract, TestingQualityRepository,
    TESTING_QUALITY_REQUIRED_METRICS,
};
use trpg_shared_kernel::{CommandEnvelope, KernelResult};

pub const PROMPT_ID: &str = "CODEX-0852-10-TESTING-QUALITY-1afba0632b";
pub const MODULE: &str = "testing_quality::readme";

pub fn contract() -> TestingQualityModuleContract {
    standard_contract(
        PROMPT_ID,
        MODULE,
        "crates/trpg-testing/src/readme.rs",
        "crates/trpg-testing/tests/readme_contract_tests.rs",
        TestingQualityAction::VerifyReadme,
        &[
            "docs/codex/10-testing-quality/README.md",
            "docs/codex/10-testing-quality/m_10_testing_quality.md",
        ],
        &[
            "readme_links_stage_s11",
            "required_testing_metrics_are_named_current_safe",
            "documentation_does_not_define_formal_state",
        ],
    )
}

pub fn required_metrics() -> &'static [&'static str] {
    TESTING_QUALITY_REQUIRED_METRICS
}

pub fn evaluate(
    repository: &mut TestingQualityRepository,
    command: &CommandEnvelope<TestingQualityCommand>,
) -> KernelResult<TestingQualityEventEnvelope> {
    evaluate_testing_quality(MODULE, repository, command)
}
