mod common;

#[test]
fn api_web_socket_contract_preserves_b029_governance() {
    common::assert_contract_governance(
        trpg_api::api_web_socket::contract(),
        "CODEX-0685-07-API-REALTIME-CONTRACTS-5d2e1fa760",
    );
}
