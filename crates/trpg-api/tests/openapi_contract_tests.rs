mod common;

#[test]
fn openapi_contract_preserves_b029_governance() {
    common::assert_contract_governance(trpg_api::openapi::contract());
}
