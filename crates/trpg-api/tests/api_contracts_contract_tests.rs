mod common;

#[test]
fn api_contracts_contract_preserves_b029_governance() {
    common::assert_contract_governance(
        trpg_api::api_contracts::contract(),
        "CODEX-0687-07-API-REALTIME-CONTRACTS-1d88035bc8",
    );
}
