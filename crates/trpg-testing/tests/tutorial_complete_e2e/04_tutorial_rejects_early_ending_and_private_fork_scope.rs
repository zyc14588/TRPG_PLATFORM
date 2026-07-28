
#[test]
fn tutorial_rejects_early_ending_and_private_fork_scope() {
    use trpg_domain_core::ddd::AuthorityMode;
    use trpg_domain_core::fork_canon_lineage::{
        calculate_snapshot_hash, fork_campaign, CampaignForkRequest, CampaignForkSnapshot,
        CanonStatus, CopyScope,
    };
    use trpg_runtime::session_runtime::{CampaignConclusion, DurableSessionState};

    assert!(
        CampaignConclusion::begin(CAMPAIGN_ID, SESSION_ID, DurableSessionState::Active).is_err(),
        "an active Session cannot be declared concluded"
    );

    let parent = trpg_test_support::authority_contract_with_owner(
        CAMPAIGN_ID,
        AuthorityMode::HumanKp,
        KEEPER_ID,
        1,
    )
    .unwrap();
    let state = r#"{"public_events":[]}"#;
    let snapshot = CampaignForkSnapshot::verified(
        CAMPAIGN_ID,
        SESSION_ID,
        state,
        calculate_snapshot_hash(state),
    )
    .unwrap();
    let request = CampaignForkRequest::new(
        CAMPAIGN_ID,
        SESSION_ID,
        CHILD_CAMPAIGN_ID,
        AuthorityMode::HumanKp,
        KEEPER_ID,
        "attempt private copy",
        snapshot.snapshot_hash.clone(),
    )
    .unwrap()
    .with_scope(
        CanonStatus::WhatIf,
        vec![CopyScope::PublicEvents, CopyScope::KeeperNotes],
    )
    .unwrap();
    assert!(
        fork_campaign(&parent, &request, &snapshot, &[]).is_err(),
        "a fork cannot opt private Keeper notes back into the copy scope"
    );
}
