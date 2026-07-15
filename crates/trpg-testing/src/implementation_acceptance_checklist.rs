use crate::{
    evaluate_testing_quality, standard_contract, TestingQualityAction, TestingQualityCommand,
    TestingQualityEventEnvelope, TestingQualityModuleContract, TestingQualityRepository,
};
use trpg_shared_kernel::{AuthorityMode, CommandEnvelope, KernelResult};

pub const PROMPT_ID: &str = "CODEX-0848-10-TESTING-QUALITY-85ad4a2b62";
pub const MODULE: &str = "testing_quality::implementation_acceptance_checklist";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcceptanceItem {
    pub id: &'static str,
    pub required: bool,
}

pub fn contract() -> TestingQualityModuleContract {
    standard_contract(
        PROMPT_ID,
        MODULE,
        "crates/trpg-testing/src/implementation_acceptance_checklist.rs",
        "crates/trpg-testing/tests/implementation_acceptance_checklist_contract_tests.rs",
        TestingQualityAction::VerifyImplementationAcceptance,
        &[
            "V1_ACCEPTANCE_EVIDENCE_MATRIX.md",
            "stages/s11-testing-quality-golden-ci/ACCEPTANCE_PROMPT.md",
        ],
        &[
            "p0_and_p1_findings_are_zero",
            "visibility_checks_are_not_weakened",
            "authority_contract_is_immutable",
        ],
    )
}

pub fn required_items() -> Vec<AcceptanceItem> {
    vec![
        AcceptanceItem {
            id: "p0_findings_allowed_zero",
            required: true,
        },
        AcceptanceItem {
            id: "p1_findings_allowed_zero",
            required: true,
        },
        AcceptanceItem {
            id: "may_weaken_tests_false",
            required: true,
        },
        AcceptanceItem {
            id: "shared_repairs_only_when_acceptance_requires",
            required: true,
        },
    ]
}

pub fn authority_contract_requires_fork() -> bool {
    let contract =
        trpg_test_support::authority_contract("campaign_testing", AuthorityMode::HumanKp, 1)
            .expect("valid contract");
    contract
        .fork_for_child(
            "campaign_testing_fork",
            AuthorityMode::AiKp,
            "ai_kp_local_level4",
        )
        .is_ok()
        && contract.mode() == &AuthorityMode::HumanKp
}

pub fn evaluate(
    repository: &mut TestingQualityRepository,
    command: &CommandEnvelope<TestingQualityCommand>,
) -> KernelResult<TestingQualityEventEnvelope> {
    evaluate_testing_quality(MODULE, repository, command)
}
