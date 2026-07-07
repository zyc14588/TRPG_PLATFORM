mod common;

#[test]
fn provider_contract_preserves_b029_governance() {
    common::assert_contract_governance(
        trpg_api::provider::contract(),
        "CODEX-0695-07-API-REALTIME-CONTRACTS-f602cf5008",
    );
}
