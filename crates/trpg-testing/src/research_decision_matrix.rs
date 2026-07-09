use crate::{
    evaluate_testing_quality, standard_contract, TestingQualityAction, TestingQualityCommand,
    TestingQualityEventEnvelope, TestingQualityModuleContract, TestingQualityRepository,
};
use trpg_shared_kernel::{CommandEnvelope, KernelResult};

pub const PROMPT_ID: &str = "CODEX-0893-10-TESTING-QUALITY-d9f4b3d265";
pub const MODULE: &str = "testing_quality::research_decision_matrix";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResearchDecisionRow {
    pub subject: &'static str,
    pub default_choice: &'static str,
    pub v1_action: &'static str,
    pub validation: &'static str,
}

pub fn contract() -> TestingQualityModuleContract {
    standard_contract(
        PROMPT_ID,
        MODULE,
        "crates/trpg-testing/src/research_decision_matrix.rs",
        "crates/trpg-testing/tests/research_decision_matrix_contract_tests.rs",
        TestingQualityAction::VerifyResearchDecisionMatrix,
        &[
            "docs/codex/99-appendix/README.md",
            "docs/codex/99-appendix/unresolved-codex-questions.md",
            "V1_ACCEPTANCE_EVIDENCE_MATRIX.md",
        ],
        &[
            "research_decisions_preserve_v1_scope",
            "provider_and_policy_choices_have_negative_tests",
            "event_store_remains_canonical_source",
        ],
    )
}

pub fn decision_rows() -> Vec<ResearchDecisionRow> {
    vec![
        ResearchDecisionRow {
            subject: "persistence",
            default_choice: "sqlx",
            v1_action: "keep_transaction_and_migration_ci",
            validation: "migration_and_contract_tests",
        },
        ResearchDecisionRow {
            subject: "event_projection_bus",
            default_choice: "nats_jetstream",
            v1_action: "publish_only_event_store_derived_events",
            validation: "replay_and_dedupe_tests",
        },
        ResearchDecisionRow {
            subject: "relationship_and_context_policy",
            default_choice: "openfga_and_opa",
            v1_action: "gate_tool_grants_and_visibility",
            validation: "allow_deny_fixture_tests",
        },
        ResearchDecisionRow {
            subject: "rag_index",
            default_choice: "pgvector",
            v1_action: "treat_index_as_rebuildable_read_model",
            validation: "rag_snapshot_rebuild_tests",
        },
        ResearchDecisionRow {
            subject: "observability",
            default_choice: "opentelemetry",
            v1_action: "redact_private_fields_in_logs_metrics_traces",
            validation: "visibility_leakage_tests",
        },
    ]
}

pub fn decision_for(subject: &str) -> Option<ResearchDecisionRow> {
    decision_rows()
        .into_iter()
        .find(|row| row.subject == subject)
}

pub fn all_rows_have_v1_validation() -> bool {
    decision_rows()
        .iter()
        .all(|row| !row.v1_action.is_empty() && !row.validation.is_empty())
}

pub fn evaluate(
    repository: &mut TestingQualityRepository,
    command: &CommandEnvelope<TestingQualityCommand>,
) -> KernelResult<TestingQualityEventEnvelope> {
    evaluate_testing_quality(MODULE, repository, command)
}
