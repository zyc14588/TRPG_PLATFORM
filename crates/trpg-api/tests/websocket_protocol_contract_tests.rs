mod common;

#[test]
fn websocket_protocol_contract_preserves_b029_governance() {
    common::assert_contract_governance(trpg_api::websocket_protocol::contract());
}
