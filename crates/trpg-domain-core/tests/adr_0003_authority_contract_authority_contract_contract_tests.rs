use trpg_domain_core::adr_0003_authority_contract_authority_contract::{
    validate_adr_0003_contract, ADR_0003_INVARIANTS,
};
use trpg_domain_core::ddd::{AuthorityMode, TrpgError};

#[test]
fn adr_0003_authority_contract_requires_locked_fork_only_contract() {
    let contract = trpg_test_support::authority_contract_with_owner(
        "camp_ai_harbor",
        AuthorityMode::AiKp,
        "ai_kp_local_level4",
        1,
    )
    .unwrap();

    assert!(ADR_0003_INVARIANTS.contains(&"authority_contract_locked"));
    validate_adr_0003_contract(&contract).unwrap();
    assert_eq!(
        trpg_test_support::authority_contract_with_owner(
            "camp_bad",
            AuthorityMode::AiKp,
            "ai_kp",
            0
        )
        .unwrap_err(),
        TrpgError::AuthorityContractMutation
    );
}
