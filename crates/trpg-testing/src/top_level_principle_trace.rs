use crate::{
    evaluate_testing_quality, standard_contract, TestingQualityAction, TestingQualityCommand,
    TestingQualityEventEnvelope, TestingQualityModuleContract, TestingQualityRepository,
};
use trpg_shared_kernel::{CommandEnvelope, KernelResult};

pub const PROMPT_ID: &str = "CODEX-0870-10-TESTING-QUALITY-0142acfd95";
pub const MODULE: &str = "testing_quality::top_level_principle_trace";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrincipleTrace {
    pub principle: &'static str,
    pub test_module: &'static str,
    pub evidence: &'static str,
}

pub fn contract() -> TestingQualityModuleContract {
    standard_contract(
        PROMPT_ID,
        MODULE,
        "crates/trpg-testing/src/top_level_principle_trace.rs",
        "crates/trpg-testing/tests/top_level_principle_trace_contract_tests.rs",
        TestingQualityAction::VerifyTopLevelPrincipleTrace,
        &[
            "docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md",
            "V1_ACCEPTANCE_EVIDENCE_MATRIX.md",
            "stages/s11-testing-quality-golden-ci/TEST_PLAN.md",
        ],
        &[
            "authority_contract_is_immutable",
            "agent_gateway_is_required",
            "event_store_is_canonical",
            "visibility_label_propagates",
        ],
    )
}

pub fn principle_traces() -> Vec<PrincipleTrace> {
    vec![
        PrincipleTrace {
            principle: "authority_contract_is_immutable",
            test_module: "trpg_domain_core::authority_immutability",
            evidence: "V1_ACCEPTANCE_EVIDENCE_MATRIX.md#authority-contract",
        },
        PrincipleTrace {
            principle: "agent_gateway_is_required",
            test_module: "trpg_agent_runtime::tool_gate",
            evidence: "V1_ACCEPTANCE_EVIDENCE_MATRIX.md#agent-decision-commit",
        },
        PrincipleTrace {
            principle: "event_store_is_canonical",
            test_module: "trpg_testing::golden_scenarios_ci",
            evidence: "fixtures/actions/golden_salt_bell_action_sequence.v1.json.md",
        },
        PrincipleTrace {
            principle: "visibility_label_propagates",
            test_module: "trpg_testing::visibility_leakage",
            evidence: "fixtures/stages/detailed/S11_golden_visibility_export_diff_expected.current.json.md",
        },
        PrincipleTrace {
            principle: "provider_boundary_is_explicit",
            test_module: "trpg_testing::model_certification_tests",
            evidence: "test-data/provider_model_certification_cases.md",
        },
    ]
}

pub fn covers_principle(principle: &str) -> bool {
    principle_traces()
        .iter()
        .any(|trace| trace.principle == principle)
}

pub fn evaluate(
    repository: &mut TestingQualityRepository,
    command: &CommandEnvelope<TestingQualityCommand>,
) -> KernelResult<TestingQualityEventEnvelope> {
    evaluate_testing_quality(MODULE, repository, command)
}
