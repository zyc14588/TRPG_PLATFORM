mod common;

#[test]
fn nats_subject_contracts_contract_preserves_b029_governance() {
    common::assert_contract_governance(
        trpg_api::nats_subject_contracts::contract(),
        "CODEX-0068-07-API-REALTIME-CONTRACTS-2b78603401",
    );
}
