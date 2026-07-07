mod common;

#[test]
fn openapi_contract_contract_preserves_b029_governance() {
    common::assert_contract_governance(
        trpg_api::openapi_contract::contract(),
        "CODEX-0698-07-API-REALTIME-CONTRACTS-a5b1a48fc3",
    );
}
