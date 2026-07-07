mod common;

#[test]
fn api_and_transport_contract_preserves_b029_governance() {
    common::assert_contract_governance(
        trpg_api::api_and_transport::contract(),
        "CODEX-0066-07-API-REALTIME-CONTRACTS-831b0504c2",
    );
}
