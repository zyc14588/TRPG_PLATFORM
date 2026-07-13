mod common;

#[test]
fn nats_subject_contracts_contract_preserves_b029_governance() {
    common::assert_contract_governance(trpg_api::nats_subject_contracts::contract());
}
