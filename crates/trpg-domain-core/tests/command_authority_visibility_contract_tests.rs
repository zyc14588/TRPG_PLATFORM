use trpg_domain_core::authority_contract::DomainAuthorityContract;
use trpg_domain_core::command_authority_visibility::validate_command_authority_visibility;
use trpg_domain_core::ddd::{
    ActorRole, AuthorityMode, DomainError, EntityId, PrincipalScope, Visibility,
};

#[test]
fn command_authority_visibility_enforces_authority_and_viewer_scope() {
    let contract = DomainAuthorityContract::new_locked(
        "camp_human_archive",
        AuthorityMode::HumanKp,
        "user_human_kp",
        1,
    )
    .unwrap();
    let user_a = EntityId::new("user_player_a").unwrap();
    let user_b = EntityId::new("user_player_b").unwrap();
    let mut command = trpg_test_support::governed_command!(
        "payload",
        ActorRole::HumanKeeper,
        AuthorityMode::HumanKp
    );
    command.visibility = Visibility::private_to_player(user_a.clone());

    assert_eq!(
        validate_command_authority_visibility(&contract, &command, &PrincipalScope::Player(user_b))
            .unwrap_err(),
        DomainError::VisibilityDenied
    );

    let accepted =
        validate_command_authority_visibility(&contract, &command, &PrincipalScope::Player(user_a))
            .unwrap();
    assert!(accepted.accepted);
}
