use crate::{
    evaluate_testing_quality, standard_contract, TestingQualityAction, TestingQualityCommand,
    TestingQualityEventEnvelope, TestingQualityModuleContract, TestingQualityRepository,
};
use trpg_shared_kernel::{CommandEnvelope, KernelResult};

pub const PROMPT_ID: &str = "CODEX-0868-10-TESTING-QUALITY-14936fa877";
pub const MODULE: &str = "testing_quality::runtime_pending_decision";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingDecisionRule {
    pub state: &'static str,
    pub may_write_canonical_state: bool,
    pub required_next_step: &'static str,
}

pub fn contract() -> TestingQualityModuleContract {
    standard_contract(
        PROMPT_ID,
        MODULE,
        "crates/trpg-testing/src/runtime_pending_decision.rs",
        "crates/trpg-testing/tests/runtime_pending_decision_contract_tests.rs",
        TestingQualityAction::VerifyRuntimePendingDecision,
        &[
            "docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md",
            "stages/s11-testing-quality-golden-ci/TEST_PLAN.md",
            "fixtures/actions/golden_salt_bell_action_sequence.v1.json.md",
        ],
        &[
            "pending_decision_cannot_write_database",
            "formal_decision_requires_event_store",
            "authority_contract_cannot_be_modified",
        ],
    )
}

pub fn pending_decision_rules() -> Vec<PendingDecisionRule> {
    vec![
        PendingDecisionRule {
            state: "ai_tool_request_pending",
            may_write_canonical_state: false,
            required_next_step: "tool_gate_decision",
        },
        PendingDecisionRule {
            state: "human_kp_review_pending",
            may_write_canonical_state: false,
            required_next_step: "command_workflow_decision",
        },
        PendingDecisionRule {
            state: "formal_decision_ready",
            may_write_canonical_state: true,
            required_next_step: "event_store_append",
        },
    ]
}

pub fn blocks_direct_agent_write() -> bool {
    pending_decision_rules()
        .iter()
        .filter(|rule| rule.state != "formal_decision_ready")
        .all(|rule| !rule.may_write_canonical_state)
}

pub fn evaluate(
    repository: &mut TestingQualityRepository,
    command: &CommandEnvelope<TestingQualityCommand>,
) -> KernelResult<TestingQualityEventEnvelope> {
    evaluate_testing_quality(MODULE, repository, command)
}
