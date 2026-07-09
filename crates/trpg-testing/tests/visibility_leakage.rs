use trpg_testing::visibility_leakage_tests;

const VISIBILITY_CASES: &str = include_str!("../../../test-data/visibility_leakage_cases.md");

#[test]
fn visibility_leakage_stage_gate() {
    assert!(VISIBILITY_CASES.contains("keeper_secret_not_in_player_export"));
    assert!(VISIBILITY_CASES.contains("private_to_player_not_party_visible"));
    assert!(VISIBILITY_CASES.contains("ai_internal_never_exported"));

    let redacted = visibility_leakage_tests::redact_player_export(
        "secret_operator keeper_truth ai_internal private_to_player",
    );

    assert!(!visibility_leakage_tests::contains_restricted_export_token(
        &redacted
    ));
}
