use crate::{
    evaluate_testing_quality, standard_contract, TestingQualityAction, TestingQualityCommand,
    TestingQualityEventEnvelope, TestingQualityModuleContract, TestingQualityRepository,
};
use trpg_shared_kernel::{CommandEnvelope, KernelResult};

pub const PROMPT_ID: &str = "CODEX-0859-10-TESTING-QUALITY-e25a4b4478";
pub const MODULE: &str = "testing_quality::implementation_acceptance_checklist_source_contract";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcceptanceSourceRule {
    pub source: &'static str,
    pub required_gate: &'static str,
}

pub fn contract() -> TestingQualityModuleContract {
    standard_contract(
        PROMPT_ID,
        MODULE,
        "crates/trpg-testing/src/implementation_acceptance_checklist_source_contract.rs",
        "crates/trpg-testing/tests/implementation_acceptance_checklist_source_contract_contract_tests.rs",
        TestingQualityAction::VerifyImplementationAcceptanceChecklistSourceContract,
        &[
            "stages/s11-testing-quality-golden-ci/ACCEPTANCE_PROMPT.md",
            "V1_ACCEPTANCE_EVIDENCE_MATRIX.md",
            "CODEX_STRICT_OPERATION_CHECKLIST.md",
        ],
        &[
            "zero_p0_p1_open_items",
            "acceptance_sources_are_current_safe",
            "visibility_and_fact_provenance_are_required",
        ],
    )
}

pub fn source_rules() -> Vec<AcceptanceSourceRule> {
    vec![
        AcceptanceSourceRule {
            source: "docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md",
            required_gate: "authority_contract_immutable",
        },
        AcceptanceSourceRule {
            source: "V1_ACCEPTANCE_EVIDENCE_MATRIX.md",
            required_gate: "p0_p1_has_evidence",
        },
        AcceptanceSourceRule {
            source: "stages/s11-testing-quality-golden-ci/ACCEPTANCE_PROMPT.md",
            required_gate: "s11_acceptance_completed",
        },
        AcceptanceSourceRule {
            source: "CODEX_STRICT_OPERATION_CHECKLIST.md",
            required_gate: "strict_operation_checklist_completed",
        },
    ]
}

pub fn requires_zero_open_p0_p1() -> bool {
    true
}

pub fn evaluate(
    repository: &mut TestingQualityRepository,
    command: &CommandEnvelope<TestingQualityCommand>,
) -> KernelResult<TestingQualityEventEnvelope> {
    evaluate_testing_quality(MODULE, repository, command)
}
