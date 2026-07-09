use crate::{
    evaluate_testing_quality, standard_contract, TestingQualityAction, TestingQualityCommand,
    TestingQualityEventEnvelope, TestingQualityModuleContract, TestingQualityRepository,
};
use trpg_shared_kernel::{CommandEnvelope, KernelResult};

pub const PROMPT_ID: &str = "CODEX-0842-10-TESTING-QUALITY-70ddb67f5e";
pub const MODULE: &str = "testing_quality::contract_test_matrix";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContractTestMatrixRow {
    pub requirement: &'static str,
    pub module: &'static str,
    pub test_file: &'static str,
}

pub fn contract() -> TestingQualityModuleContract {
    standard_contract(
        PROMPT_ID,
        MODULE,
        "crates/trpg-testing/src/contract_test_matrix.rs",
        "crates/trpg-testing/tests/contract_test_matrix_contract_tests.rs",
        TestingQualityAction::ValidateContractTestMatrix,
        &[
            "docs/codex/10-testing-quality/contract_test_matrix.md",
            "stages/s11-testing-quality-golden-ci/TEST_PLAN.md",
        ],
        &[
            "each_primary_module_has_contract_test",
            "governance_red_lines_are_mapped_to_tests",
            "supplemental_rows_merge_into_primary_tests",
        ],
    )
}

pub fn matrix_rows() -> Vec<ContractTestMatrixRow> {
    crate::primary_contracts()
        .into_iter()
        .map(|contract| ContractTestMatrixRow {
            requirement: contract.action.as_str(),
            module: contract.module,
            test_file: contract.test_file,
        })
        .collect()
}

pub fn evaluate(
    repository: &mut TestingQualityRepository,
    command: &CommandEnvelope<TestingQualityCommand>,
) -> KernelResult<TestingQualityEventEnvelope> {
    evaluate_testing_quality(MODULE, repository, command)
}
