use trpg_domain_core::ddd::{EntityId, PrincipalScope, Visibility, VisibilityLabel};
use trpg_domain_core::visibility_enforcement_points::{
    enforce_visibility_at, VisibilityEnforcementPoint,
};

#[test]
fn visibility_enforcement_points_deny_restricted_summary() {
    let visibility = Visibility::new(VisibilityLabel::KeeperOnly);

    assert!(enforce_visibility_at(
        VisibilityEnforcementPoint::Summary,
        &visibility,
        &PrincipalScope::PartyMember
    )
    .is_err());
}

#[test]
fn visibility_enforcement_points_allow_matching_private_player() {
    let player = EntityId::new("player_a").unwrap();
    let visibility = Visibility::private_to_player(player.clone());

    assert!(enforce_visibility_at(
        VisibilityEnforcementPoint::ApiResponse,
        &visibility,
        &PrincipalScope::Player(player)
    )
    .is_ok());
}
