use trpg_shared_kernel::{EntityId, PrincipalScope, Visibility, VisibilityLabel};
use trpg_testing::visibility_leakage_tests::PlayerExportEvaluation;
use trpg_testing::{record_contract_decision, visibility_leakage_tests};

const VISIBILITY_CASES: &str = include_str!("../../../test-data/visibility_leakage_cases.md");
const GOLDEN_EXPECTED: &str = include_str!(
    "../../../fixtures/stages/detailed/S11_golden_visibility_export_diff_expected.current.json.md"
);

#[test]
fn visibility_leakage_uses_typed_policy_instead_of_keyword_replacement() {
    record_contract_decision(&visibility_leakage_tests::contract()).expect("recorded");

    for case_id in [
        "keeper_secret_not_in_player_export",
        "private_to_player_not_party_visible",
        "ai_internal_never_exported",
    ] {
        assert!(VISIBILITY_CASES.contains(case_id));
    }
    assert!(GOLDEN_EXPECTED.contains("VISIBILITY_LEAKAGE_DETECTED"));

    let player = PrincipalScope::Player(EntityId::new("player_a").unwrap());
    let secret_text = "ordinary prose with no policy keywords";
    assert_eq!(
        visibility_leakage_tests::evaluate_player_export(
            secret_text,
            Some(&Visibility::new(VisibilityLabel::KeeperOnly)),
            &PrincipalScope::System,
            &player,
        ),
        PlayerExportEvaluation::Redacted
    );
    assert_eq!(
        visibility_leakage_tests::evaluate_player_export(
            "keeper_truth is harmless public prose here",
            Some(&Visibility::new(VisibilityLabel::Public)),
            &PrincipalScope::System,
            &player,
        ),
        PlayerExportEvaluation::Visible("keeper_truth is harmless public prose here".to_owned())
    );
    assert_eq!(
        visibility_leakage_tests::evaluate_player_export(
            secret_text,
            None,
            &PrincipalScope::System,
            &player,
        ),
        PlayerExportEvaluation::NotEvaluated
    );
    assert_eq!(visibility_leakage_tests::fixture_cases().len(), 3);
}
