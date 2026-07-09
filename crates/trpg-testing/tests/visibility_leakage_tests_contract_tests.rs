use trpg_testing::{record_contract_decision, visibility_leakage_tests};

const VISIBILITY_CASES: &str = include_str!("../../../test-data/visibility_leakage_cases.md");
const GOLDEN_EXPECTED: &str = include_str!(
    "../../../fixtures/stages/detailed/S11_golden_visibility_export_diff_expected.current.json.md"
);

#[test]
fn visibility_leakage_redacts_restricted_export_tokens() {
    record_contract_decision(&visibility_leakage_tests::contract()).expect("recorded");

    for case_id in [
        "keeper_secret_not_in_player_export",
        "private_to_player_not_party_visible",
        "ai_internal_never_exported",
    ] {
        assert!(VISIBILITY_CASES.contains(case_id));
    }
    assert!(GOLDEN_EXPECTED.contains("VISIBILITY_LEAKAGE_DETECTED"));

    let redacted = visibility_leakage_tests::redact_player_export(
        "secret_operator keeper_truth ai_internal private_to_player",
    );

    assert!(!visibility_leakage_tests::contains_restricted_export_token(
        &redacted
    ));
    assert_eq!(visibility_leakage_tests::fixture_cases().len(), 3);
}
