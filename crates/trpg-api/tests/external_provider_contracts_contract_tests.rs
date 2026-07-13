mod common;

#[test]
fn external_provider_contracts_contract_preserves_b029_governance() {
    common::assert_contract_governance(trpg_api::external_provider_contracts::contract());
}
