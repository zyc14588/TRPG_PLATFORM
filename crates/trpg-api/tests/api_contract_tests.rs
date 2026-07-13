mod common;

#[test]
fn api_contract_preserves_b029_governance() {
    common::assert_contract_governance(trpg_api::api::contract());
}
