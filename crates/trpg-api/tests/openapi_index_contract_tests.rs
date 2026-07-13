mod common;

#[test]
fn openapi_index_contract_preserves_b029_governance() {
    common::assert_contract_governance(trpg_api::openapi_index::contract());
}
