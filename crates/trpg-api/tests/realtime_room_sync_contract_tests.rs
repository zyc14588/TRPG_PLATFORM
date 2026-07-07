mod common;

#[test]
fn realtime_room_sync_contract_preserves_b029_governance() {
    common::assert_contract_governance(
        trpg_api::realtime_room_sync::contract(),
        "CODEX-0686-07-API-REALTIME-CONTRACTS-54d06d623d",
    );
}
