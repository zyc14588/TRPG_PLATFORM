use trpg_domain_core::authority_contract::{
    patch_locked_authority_contract, ChangePolicy, DomainAuthorityContract,
};
use trpg_domain_core::ddd::{AuthorityMode, DomainError};

#[test]
fn authority_contract_rejects_in_place_mode_or_owner_change() {
    let contract = DomainAuthorityContract::new_locked(
        "camp_human_archive",
        AuthorityMode::HumanKp,
        "user_human_kp",
        1,
    )
    .unwrap();

    let error =
        patch_locked_authority_contract(&contract, AuthorityMode::AiKp, "ai_kp_local_level4")
            .unwrap_err();

    assert_eq!(error, DomainError::AuthorityContractImmutable);
    assert_eq!(error.code(), "AUTHORITY_CONTRACT_IMMUTABLE");
}

#[test]
fn authority_contract_fork_creates_locked_child_without_mutating_parent() {
    let parent = DomainAuthorityContract::new_locked(
        "camp_ai_harbor",
        AuthorityMode::AiKp,
        "ai_kp_local_level4",
        1,
    )
    .unwrap();

    let child = parent
        .fork_for_child(
            "camp_human_harbor_whatif",
            AuthorityMode::HumanKp,
            "user_human_kp",
        )
        .unwrap();

    assert_eq!(parent.authority_mode(), &AuthorityMode::AiKp);
    assert_eq!(child.authority_mode(), &AuthorityMode::HumanKp);
    assert_eq!(parent.authority_owner().as_str(), "ai_kp_local_level4");
    assert_eq!(child.authority_owner().as_str(), "user_human_kp");
    assert!(child.is_locked());
    assert_eq!(child.change_policy(), ChangePolicy::ForkOnly);
    assert_eq!(child.version(), 1);
}

#[test]
fn authority_contract_exposes_only_read_only_accessors() {
    let contract = DomainAuthorityContract::new_locked(
        "camp_ai_harbor",
        AuthorityMode::AiKp,
        "ai_kp_local_level4",
        1,
    )
    .unwrap();

    assert!(contract.contract_id().as_str().contains("camp_ai_harbor"));
    assert_eq!(contract.campaign_id().as_str(), "camp_ai_harbor");
    assert_eq!(contract.authority_mode(), &AuthorityMode::AiKp);
    assert_eq!(contract.authority_owner().as_str(), "ai_kp_local_level4");
    assert_eq!(contract.version(), 1);
    assert!(contract.is_locked());
    assert_eq!(contract.change_policy(), ChangePolicy::ForkOnly);
}
