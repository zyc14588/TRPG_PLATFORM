mod common;

#[test]
fn realtime_room_sync_contract_preserves_b029_governance() {
    common::assert_contract_governance(trpg_api::realtime_room_sync::contract());
}
