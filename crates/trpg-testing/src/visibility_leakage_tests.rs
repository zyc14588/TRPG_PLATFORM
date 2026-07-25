use crate::{
    evaluate_testing_quality, standard_contract, TestingQualityAction, TestingQualityCommand,
    TestingQualityEventEnvelope, TestingQualityModuleContract, TestingQualityRepository,
};
use trpg_security_governance::{
    evaluate_derived_visibility, DerivationRequest, DerivedObject, RedactionOutcome,
};
use trpg_shared_kernel::{
    CommandEnvelope, KernelResult, PrincipalScope, Visibility, VisibilityKind,
};

pub const PROMPT_ID: &str = "CODEX-0095-10-TESTING-QUALITY-e84e4a394d";
pub const MODULE: &str = "testing_quality::visibility_leakage_tests";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VisibilityLeakageCase {
    pub case_id: &'static str,
    pub source_label: VisibilityKind,
    pub expected: &'static str,
}

pub fn contract() -> TestingQualityModuleContract {
    standard_contract(
        PROMPT_ID,
        MODULE,
        "crates/trpg-testing/src/visibility_leakage_tests.rs",
        "crates/trpg-testing/tests/visibility_leakage_tests_contract_tests.rs",
        TestingQualityAction::VerifyVisibilityLeakage,
        &[
            "test-data/visibility_leakage_cases.md",
            "fixtures/stages/detailed/S11_golden_visibility_export_diff_expected.current.json.md",
        ],
        &[
            "keeper_secret_not_in_player_export",
            "private_to_player_not_party_visible",
            "ai_internal_never_exported",
        ],
    )
}

pub fn fixture_cases() -> Vec<VisibilityLeakageCase> {
    vec![
        VisibilityLeakageCase {
            case_id: "keeper_secret_not_in_player_export",
            source_label: VisibilityKind::KeeperOnly,
            expected: "REDACTED",
        },
        VisibilityLeakageCase {
            case_id: "private_to_player_not_party_visible",
            source_label: VisibilityKind::PrivateToPlayer,
            expected: "REDACTED",
        },
        VisibilityLeakageCase {
            case_id: "ai_internal_never_exported",
            source_label: VisibilityKind::AiInternal,
            expected: "REDACTED_OR_AUDIT_ONLY",
        },
    ]
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlayerExportEvaluation {
    Visible(String),
    Redacted,
    Omitted,
    NotEvaluated,
}

/// Evaluates exportability exclusively from trusted visibility metadata and
/// the declared target audience. Text content is never treated as a policy
/// signal; missing classification remains `NotEvaluated` and exposes nothing.
pub fn evaluate_player_export(
    text: &str,
    visibility: Option<&Visibility>,
    processor: &PrincipalScope,
    target_audience: &PrincipalScope,
) -> PlayerExportEvaluation {
    let Some(visibility) = visibility else {
        return PlayerExportEvaluation::NotEvaluated;
    };
    let sources = [visibility.clone()];
    match evaluate_derived_visibility(DerivationRequest {
        sources: &sources,
        processor,
        target_audience,
        target: DerivedObject::PlayerExport,
    })
    .outcome
    {
        RedactionOutcome::Visible => PlayerExportEvaluation::Visible(text.to_owned()),
        RedactionOutcome::Redacted => PlayerExportEvaluation::Redacted,
        RedactionOutcome::Omitted => PlayerExportEvaluation::Omitted,
    }
}

pub fn evaluate(
    repository: &mut TestingQualityRepository,
    command: &CommandEnvelope<TestingQualityCommand>,
) -> KernelResult<TestingQualityEventEnvelope> {
    evaluate_testing_quality(MODULE, repository, command)
}
