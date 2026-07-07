mod common;

#[test]
fn api_web_socket_g_rpc_schema_contract_preserves_b029_governance() {
    common::assert_contract_governance(
        trpg_api::api_web_socket_g_rpc_schema::contract(),
        "CODEX-0688-07-API-REALTIME-CONTRACTS-991d938d5b",
    );
}
