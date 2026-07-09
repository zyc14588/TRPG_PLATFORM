use crate::{
    evaluate_testing_quality, standard_contract, TestingQualityAction, TestingQualityCommand,
    TestingQualityEventEnvelope, TestingQualityModuleContract, TestingQualityRepository,
};
use trpg_shared_kernel::{CommandEnvelope, KernelResult};

pub const PROMPT_ID: &str = "CODEX-0892-10-TESTING-QUALITY-1b68a77fb7";
pub const MODULE: &str = "testing_quality::latest_deep_research_rust_summary";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResearchSummaryDecision {
    pub topic: &'static str,
    pub selected: &'static str,
    pub validation: &'static str,
}

pub fn contract() -> TestingQualityModuleContract {
    standard_contract(
        PROMPT_ID,
        MODULE,
        "crates/trpg-testing/src/latest_deep_research_rust_summary.rs",
        "crates/trpg-testing/tests/latest_deep_research_rust_summary_contract_tests.rs",
        TestingQualityAction::VerifyLatestDeepResearchRustSummary,
        &[
            "docs/codex/99-appendix/README.md",
            "docs/codex/99-appendix/codex-official-reference-notes.md",
            "stages/s11-testing-quality-golden-ci/TEST_PLAN.md",
        ],
        &[
            "rust_first_decisions_are_current_safe",
            "selected_defaults_have_validation_commands",
            "research_summary_does_not_expand_runtime_scope",
        ],
    )
}

pub fn research_summary_decisions() -> Vec<ResearchSummaryDecision> {
    vec![
        ResearchSummaryDecision {
            topic: "database_access",
            selected: "sqlx",
            validation: "cargo test -p trpg-testing --all-features",
        },
        ResearchSummaryDecision {
            topic: "event_bus",
            selected: "nats_jetstream",
            validation: "replay_contract_tests",
        },
        ResearchSummaryDecision {
            topic: "authorization_policy",
            selected: "openfga_and_opa",
            validation: "deny_and_visibility_negative_cases",
        },
        ResearchSummaryDecision {
            topic: "canonical_state",
            selected: "event_store",
            validation: "projection_rebuild_and_replay",
        },
    ]
}

pub fn selected_for(topic: &str) -> Option<&'static str> {
    research_summary_decisions()
        .into_iter()
        .find(|decision| decision.topic == topic)
        .map(|decision| decision.selected)
}

pub fn all_decisions_have_validation() -> bool {
    research_summary_decisions()
        .iter()
        .all(|decision| !decision.validation.is_empty())
}

pub fn evaluate(
    repository: &mut TestingQualityRepository,
    command: &CommandEnvelope<TestingQualityCommand>,
) -> KernelResult<TestingQualityEventEnvelope> {
    evaluate_testing_quality(MODULE, repository, command)
}
