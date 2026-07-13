mod common;

#[test]
fn realtime_sync_contract_preserves_b029_governance() {
    common::assert_contract_governance(trpg_api::realtime_sync::contract());
}
