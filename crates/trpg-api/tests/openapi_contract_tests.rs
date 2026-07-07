mod common;

#[test]
fn openapi_contract_preserves_b029_governance() {
    common::assert_contract_governance(
        trpg_api::openapi::contract(),
        "CODEX-0689-07-API-REALTIME-CONTRACTS-4b17a0fb09",
    );
}
