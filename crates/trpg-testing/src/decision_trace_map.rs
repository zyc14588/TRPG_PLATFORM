use crate::{
    evaluate_testing_quality, standard_contract, TestingQualityAction, TestingQualityCommand,
    TestingQualityEventEnvelope, TestingQualityModuleContract, TestingQualityRepository,
};
use trpg_shared_kernel::{CommandEnvelope, KernelResult};

pub const PROMPT_ID: &str = "CODEX-0839-10-TESTING-QUALITY-09775e3a7b";
pub const MODULE: &str = "testing_quality::decision_trace_map";

pub const BATCH_038_PROMPT_IDS: &[&str] = &[
    "CODEX-0088-10-TESTING-QUALITY-20897e8633",
    "CODEX-0089-10-TESTING-QUALITY-da28af3028",
    "CODEX-0090-10-TESTING-QUALITY-db69d85d0f",
    "CODEX-0091-10-TESTING-QUALITY-6730499fe0",
    "CODEX-0092-10-TESTING-QUALITY-d6a006e0a1",
    "CODEX-0093-10-TESTING-QUALITY-97f7f731a8",
    "CODEX-0094-10-TESTING-QUALITY-6ac95ec41f",
    "CODEX-0095-10-TESTING-QUALITY-e84e4a394d",
    "CODEX-0839-10-TESTING-QUALITY-09775e3a7b",
    "CODEX-0840-10-TESTING-QUALITY-069d3f779b",
    "CODEX-0841-10-TESTING-QUALITY-661dfc0224",
    "CODEX-0842-10-TESTING-QUALITY-70ddb67f5e",
    "CODEX-0843-10-TESTING-QUALITY-a8f283084f",
    "CODEX-0844-10-TESTING-QUALITY-be04cff75f",
    "CODEX-0845-10-TESTING-QUALITY-6254d78940",
    "CODEX-0846-10-TESTING-QUALITY-4191e3f193",
    "CODEX-0847-10-TESTING-QUALITY-923fc94916",
    "CODEX-0848-10-TESTING-QUALITY-85ad4a2b62",
    "CODEX-0849-10-TESTING-QUALITY-3b745596ac",
    "CODEX-0850-10-TESTING-QUALITY-f5b7059f4f",
    "CODEX-0851-10-TESTING-QUALITY-c4d5125cc0",
    "CODEX-0852-10-TESTING-QUALITY-1afba0632b",
    "CODEX-0853-10-TESTING-QUALITY-eaf9de3475",
    "CODEX-0854-10-TESTING-QUALITY-705d02fdf8",
    "CODEX-0855-10-TESTING-QUALITY-0adc8f6280",
];

pub const BATCH_039_PROMPT_IDS: &[&str] = &[
    "CODEX-0856-10-TESTING-QUALITY-78184e52c9",
    "CODEX-0857-10-TESTING-QUALITY-2aa3aea6d1",
    "CODEX-0858-10-TESTING-QUALITY-dae7b4dc49",
    "CODEX-0859-10-TESTING-QUALITY-e25a4b4478",
    "CODEX-0860-10-TESTING-QUALITY-0de1e4e40c",
    "CODEX-0861-10-TESTING-QUALITY-2664a3d8ee",
    "CODEX-0862-10-TESTING-QUALITY-fdd4d14b4b",
    "CODEX-0863-10-TESTING-QUALITY-0c4693f8ae",
    "CODEX-0864-10-TESTING-QUALITY-6cb8a48c80",
    "CODEX-0865-10-TESTING-QUALITY-aea366b339",
    "CODEX-0866-10-TESTING-QUALITY-897bc79dc9",
    "CODEX-0867-10-TESTING-QUALITY-7667081407",
    "CODEX-0868-10-TESTING-QUALITY-14936fa877",
    "CODEX-0869-10-TESTING-QUALITY-20c4bd1d75",
    "CODEX-0870-10-TESTING-QUALITY-0142acfd95",
    "CODEX-0871-10-TESTING-QUALITY-fd5bd618c9",
    "CODEX-0872-10-TESTING-QUALITY-37373a8f49",
    "CODEX-0873-10-TESTING-QUALITY-17111d90f9",
    "CODEX-0874-10-TESTING-QUALITY-95e0ac6e0d",
    "CODEX-0875-10-TESTING-QUALITY-a2e797e671",
    "CODEX-0876-10-TESTING-QUALITY-ad4716763d",
    "CODEX-0877-10-TESTING-QUALITY-5a0fb801cc",
    "CODEX-0878-10-TESTING-QUALITY-6d59753ce7",
    "CODEX-0879-10-TESTING-QUALITY-b0eba279f4",
    "CODEX-0880-10-TESTING-QUALITY-cc964ce88c",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecisionTraceRow {
    pub prompt_id: &'static str,
    pub module: &'static str,
    pub output: &'static str,
}

pub fn contract() -> TestingQualityModuleContract {
    standard_contract(
        PROMPT_ID,
        MODULE,
        "crates/trpg-testing/src/decision_trace_map.rs",
        "crates/trpg-testing/tests/decision_trace_map_contract_tests.rs",
        TestingQualityAction::VerifyDecisionTraceMap,
        &[
            "docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md",
            "docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md",
        ],
        &[
            "all_batch_prompts_are_traced",
            "supplemental_prompts_do_not_own_rust",
            "current_safe_module_names_are_used",
        ],
    )
}

pub fn primary_trace_rows() -> Vec<DecisionTraceRow> {
    crate::primary_contracts()
        .into_iter()
        .map(|contract| DecisionTraceRow {
            prompt_id: contract.prompt_id,
            module: contract.module,
            output: contract.source_file,
        })
        .collect()
}

pub fn evaluate(
    repository: &mut TestingQualityRepository,
    command: &CommandEnvelope<TestingQualityCommand>,
) -> KernelResult<TestingQualityEventEnvelope> {
    evaluate_testing_quality(MODULE, repository, command)
}
