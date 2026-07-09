use crate::{
    evaluate_testing_quality, standard_contract, TestingQualityAction, TestingQualityCommand,
    TestingQualityEventEnvelope, TestingQualityModuleContract, TestingQualityRepository,
};
use trpg_shared_kernel::{CommandEnvelope, KernelResult};

pub const PROMPT_ID: &str = "CODEX-0871-10-TESTING-QUALITY-fd5bd618c9";
pub const MODULE: &str = "testing_quality::principle_to_doc_trace";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrincipleDocLink {
    pub principle: &'static str,
    pub document: &'static str,
    pub test_module: &'static str,
}

pub fn contract() -> TestingQualityModuleContract {
    standard_contract(
        PROMPT_ID,
        MODULE,
        "crates/trpg-testing/src/principle_to_doc_trace.rs",
        "crates/trpg-testing/tests/principle_to_doc_trace_contract_tests.rs",
        TestingQualityAction::VerifyPrincipleToDocTrace,
        &[
            "docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md",
            "docs/codex/10-testing-quality/contract_test_matrix.md",
            "docs/codex/10-testing-quality/README.md",
        ],
        &[
            "top_level_principles_have_current_docs",
            "testing_quality_docs_reference_stage_tests",
            "docs_do_not_create_implementation_scope",
        ],
    )
}

pub fn doc_links() -> Vec<PrincipleDocLink> {
    vec![
        PrincipleDocLink {
            principle: "authority_contract_is_immutable",
            document: "docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md",
            test_module: "implementation_acceptance_checklist_source_contract",
        },
        PrincipleDocLink {
            principle: "visibility_label_propagates",
            document: "docs/codex/10-testing-quality/contract_test_matrix.md",
            test_module: "visibility_leakage",
        },
        PrincipleDocLink {
            principle: "event_store_is_canonical",
            document: "docs/codex/10-testing-quality/README.md",
            test_module: "golden_scenarios_ci",
        },
        PrincipleDocLink {
            principle: "provider_boundary_is_explicit",
            document: "stages/s11-testing-quality-golden-ci/TEST_PLAN.md",
            test_module: "model_certification_tests",
        },
    ]
}

pub fn links_current_doc(document: &str) -> bool {
    doc_links().iter().any(|link| link.document == document)
}

pub fn evaluate(
    repository: &mut TestingQualityRepository,
    command: &CommandEnvelope<TestingQualityCommand>,
) -> KernelResult<TestingQualityEventEnvelope> {
    evaluate_testing_quality(MODULE, repository, command)
}
