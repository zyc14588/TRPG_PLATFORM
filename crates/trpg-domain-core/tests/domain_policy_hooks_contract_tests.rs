use trpg_domain_core::authority_contract::DomainAuthorityContract;
use trpg_domain_core::ddd::{
    ActorRole, AuthorityMode, CommandEnvelope, DomainError, EntityId, PrincipalScope, Visibility,
    VisibilityLabel,
};
use trpg_domain_core::domain_policy_hooks::{
    allow_governed_command, deny_by_default, PolicyContext, PolicyDecision,
};

#[test]
fn policy_hooks_default_deny_and_allow_only_governed_visible_commands() {
    assert_eq!(
        deny_by_default(),
        PolicyDecision::Deny(DomainError::PolicyDenied)
    );

    let contract = DomainAuthorityContract::new_locked(
        "camp_human_archive",
        AuthorityMode::HumanKp,
        "user_human_kp",
        1,
    )
    .unwrap();
    let command =
        CommandEnvelope::governed("payload", ActorRole::HumanKeeper, AuthorityMode::HumanKp);

    let denied = allow_governed_command(
        &contract,
        &command,
        &PolicyContext {
            principal: PrincipalScope::Player(EntityId::new("user_player_b").unwrap()),
            target_visibility: Visibility::new(VisibilityLabel::KeeperOnly),
        },
    )
    .unwrap();
    assert_eq!(denied, PolicyDecision::Deny(DomainError::VisibilityDenied));

    let allowed = allow_governed_command(
        &contract,
        &command,
        &PolicyContext {
            principal: PrincipalScope::Keeper,
            target_visibility: Visibility::new(VisibilityLabel::KeeperOnly),
        },
    )
    .unwrap();
    assert_eq!(allowed, PolicyDecision::Allow);
}
