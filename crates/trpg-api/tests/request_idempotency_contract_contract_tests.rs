mod common;

#[test]
fn request_idempotency_contract_preserves_b029_governance() {
    common::assert_contract_governance(trpg_api::request_idempotency_contract::contract());
}
