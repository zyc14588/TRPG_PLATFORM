use trpg_domain_core::ddd::{AuthorityMode, DomainError};
use trpg_domain_core::fork_canon_lineage::{
    calculate_snapshot_hash, fork_campaign, CampaignForkRequest, CampaignForkSnapshot, CanonStatus,
    CopyScope,
};

fn public_snapshot() -> CampaignForkSnapshot {
    let canonical_state_json = r#"{"characters":[],"events":[],"keeper_notes":[]}"#;
    CampaignForkSnapshot::verified(
        "camp_ai_harbor",
        "session_002",
        canonical_state_json,
        calculate_snapshot_hash(canonical_state_json),
    )
    .unwrap()
}

#[test]
fn fork_canon_lineage_copies_verified_public_state_and_locks_child_contract() {
    let parent = trpg_test_support::authority_contract_with_owner(
        "camp_ai_harbor",
        AuthorityMode::AiKp,
        "ai_kp_local_level4",
        1,
    )
    .unwrap();
    let snapshot = public_snapshot();
    let request = CampaignForkRequest::new(
        "camp_ai_harbor",
        "session_002",
        "camp_human_harbor_whatif",
        AuthorityMode::HumanKp,
        "user_human_kp",
        "player_requested_human_kp_branch",
        snapshot.snapshot_hash.clone(),
    )
    .unwrap();

    let fork = fork_campaign(&parent, &request, &snapshot, &[]).unwrap();

    assert!(fork.parent_unchanged);
    assert_eq!(fork.canon_status, CanonStatus::WhatIf);
    assert_eq!(
        fork.child_authority_contract.authority_mode(),
        &AuthorityMode::HumanKp
    );
    assert!(fork.child_authority_contract.is_locked());
    assert!(fork.copied_scopes.contains(&CopyScope::PublicEvents));
    assert!(fork
        .excluded_private_scopes
        .contains(&CopyScope::AiInternalMemory));
    assert_eq!(fork.copied_snapshot, snapshot);
    assert_eq!(parent.authority_mode(), &AuthorityMode::AiKp);
}

#[test]
fn fork_rejects_self_fork_invalid_or_mismatched_snapshot_and_private_scope() {
    let parent = trpg_test_support::authority_contract_with_owner(
        "camp_ai_harbor",
        AuthorityMode::AiKp,
        "ai_kp_local_level4",
        1,
    )
    .unwrap();
    let snapshot = public_snapshot();

    assert!(CampaignForkRequest::new(
        "camp_ai_harbor",
        "session_002",
        "camp_ai_harbor",
        AuthorityMode::AiKp,
        "ai_kp_local_level4",
        "self fork",
        snapshot.snapshot_hash.clone(),
    )
    .is_err());
    assert!(CampaignForkRequest::new(
        "camp_ai_harbor",
        "session_002",
        "camp_other",
        AuthorityMode::AiKp,
        "ai_kp_local_level4",
        "",
        "not-a-hash",
    )
    .is_err());

    let wrong_hash = format!("sha256:{}", "0".repeat(64));
    let mismatched = CampaignForkRequest::new(
        "camp_ai_harbor",
        "session_002",
        "camp_other",
        AuthorityMode::AiKp,
        "ai_kp_local_level4",
        "hash mismatch",
        wrong_hash,
    )
    .unwrap();
    assert_eq!(
        fork_campaign(&parent, &mismatched, &snapshot, &[]).unwrap_err(),
        DomainError::AuthorityViolation
    );

    let private_request = CampaignForkRequest::new(
        "camp_ai_harbor",
        "session_002",
        "camp_private",
        AuthorityMode::HumanKp,
        "user_human_kp",
        "private request",
        snapshot.snapshot_hash.clone(),
    )
    .unwrap()
    .with_scope(
        CanonStatus::NonCanon,
        vec![CopyScope::PublicEvents, CopyScope::KeeperNotes],
    )
    .unwrap();
    assert_eq!(
        fork_campaign(&parent, &private_request, &snapshot, &[]).unwrap_err(),
        DomainError::VisibilityDenied
    );
    assert!(fork_campaign(
        &parent,
        &private_request,
        &snapshot,
        &[CopyScope::KeeperNotes]
    )
    .is_ok());
}
