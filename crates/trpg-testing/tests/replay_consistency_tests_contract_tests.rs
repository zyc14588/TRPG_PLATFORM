use trpg_testing::{
    record_contract_decision, replay_consistency_tests, TESTING_QUALITY_DECISION_RECORDED_EVENT,
};

const EXPORT_SNAPSHOT_DATA: &str = include_str!("../../../test-data/export_expected_snapshots.md");

#[test]
fn replay_consistency_rebuilds_projection_from_event_store() {
    let (repository, _, event) =
        record_contract_decision(&replay_consistency_tests::contract()).expect("recorded");

    assert_eq!(event.event_type, TESTING_QUALITY_DECISION_RECORDED_EVENT);
    assert!(EXPORT_SNAPSHOT_DATA.contains("player_export"));
    assert!(EXPORT_SNAPSHOT_DATA.contains("keeper_export"));

    let projection = replay_consistency_tests::rebuild_projection(repository.events());

    assert_eq!(projection.event_count, repository.events().len());
    assert_ne!(projection.final_state_hash, 0);
}

#[test]
fn replay_digest_is_deterministic_for_same_event_history() {
    let (left_repository, _, _) =
        record_contract_decision(&replay_consistency_tests::contract()).expect("recorded");
    let (right_repository, _, _) =
        record_contract_decision(&replay_consistency_tests::contract()).expect("recorded");

    assert_eq!(
        replay_consistency_tests::replay_digest(left_repository.events()),
        replay_consistency_tests::replay_digest(right_repository.events())
    );
}
