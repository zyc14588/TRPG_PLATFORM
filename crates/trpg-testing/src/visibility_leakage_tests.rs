use crate::{
    evaluate_testing_quality, standard_contract, TestingQualityAction, TestingQualityCommand,
    TestingQualityEventEnvelope, TestingQualityModuleContract, TestingQualityRepository,
};
use trpg_shared_kernel::{CommandEnvelope, KernelResult, VisibilityLabel};

pub const PROMPT_ID: &str = "CODEX-0095-10-TESTING-QUALITY-e84e4a394d";
pub const MODULE: &str = "testing_quality::visibility_leakage_tests";

pub const RESTRICTED_EXPORT_TOKENS: &[&str] = &[
    "secret_operator",
    "keeper_truth",
    "ai_internal",
    "keeper_only",
    "private_to_player",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VisibilityLeakageCase {
    pub case_id: &'static str,
    pub source_label: VisibilityLabel,
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
            source_label: VisibilityLabel::KeeperOnly,
            expected: "REDACTED",
        },
        VisibilityLeakageCase {
            case_id: "private_to_player_not_party_visible",
            source_label: VisibilityLabel::PrivateToPlayer,
            expected: "REDACTED",
        },
        VisibilityLeakageCase {
            case_id: "ai_internal_never_exported",
            source_label: VisibilityLabel::AiInternal,
            expected: "REDACTED_OR_AUDIT_ONLY",
        },
    ]
}

pub fn redact_player_export(text: &str) -> String {
    RESTRICTED_EXPORT_TOKENS
        .iter()
        .fold(text.to_owned(), |redacted, token| {
            redacted.replace(token, "[redacted]")
        })
}

pub fn contains_restricted_export_token(text: &str) -> bool {
    RESTRICTED_EXPORT_TOKENS
        .iter()
        .any(|token| text.contains(token))
}

pub fn evaluate(
    repository: &mut TestingQualityRepository,
    command: &CommandEnvelope<TestingQualityCommand>,
) -> KernelResult<TestingQualityEventEnvelope> {
    evaluate_testing_quality(MODULE, repository, command)
}
