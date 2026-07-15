use trpg_domain_core::ddd::AuthorityMode;
use trpg_domain_core::fork_canon_lineage::{
    fork_campaign, CampaignForkRequest, CanonStatus, CopyScope,
};

#[test]
fn fork_canon_lineage_copies_public_state_and_locks_child_contract() {
    let parent = trpg_test_support::authority_contract_with_owner(
        "camp_ai_harbor",
        AuthorityMode::AiKp,
        "ai_kp_local_level4",
        1,
    )
    .unwrap();
    let request = CampaignForkRequest::new(
        "camp_ai_harbor",
        "session_002",
        "camp_human_harbor_whatif",
        AuthorityMode::HumanKp,
        "user_human_kp",
        "player_requested_human_kp_branch",
        "sha256_9e5d1b0c",
    )
    .unwrap();

    let fork = fork_campaign(&parent, &request).unwrap();

    assert!(fork.parent_unchanged);
    assert_eq!(fork.canon_status, CanonStatus::WhatIf);
    assert_eq!(
        fork.child_authority_contract.authority_mode(),
        &AuthorityMode::HumanKp
    );
    assert!(fork.child_authority_contract.is_locked());
    assert!(fork.copied_by_default.contains(&CopyScope::PublicEvents));
    assert!(fork
        .requires_explicit_permission
        .contains(&CopyScope::AiInternalMemory));
}
