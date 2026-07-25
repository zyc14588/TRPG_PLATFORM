use trpg_shared_kernel::{EntityId, PrincipalScope, Visibility, VisibilityLabel};
use trpg_testing::visibility_leakage_tests;
use trpg_testing::visibility_leakage_tests::PlayerExportEvaluation;

const VISIBILITY_CASES: &str = include_str!("../../../test-data/visibility_leakage_cases.md");

#[test]
fn visibility_leakage_stage_gate() {
    assert!(VISIBILITY_CASES.contains("keeper_secret_not_in_player_export"));
    assert!(VISIBILITY_CASES.contains("private_to_player_not_party_visible"));
    assert!(VISIBILITY_CASES.contains("ai_internal_never_exported"));

    let player = PrincipalScope::Player(EntityId::new("player_a").unwrap());
    assert_eq!(
        visibility_leakage_tests::evaluate_player_export(
            "content without a magic sensitive token",
            Some(&Visibility::new(VisibilityLabel::KeeperOnly)),
            &PrincipalScope::System,
            &player,
        ),
        PlayerExportEvaluation::Redacted
    );
}
