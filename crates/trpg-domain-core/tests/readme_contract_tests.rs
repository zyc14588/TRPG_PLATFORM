use trpg_domain_core::readme::domain_core_readme_contract;

#[test]
fn readme_contract_lists_domain_core_boundaries() {
    let contract = domain_core_readme_contract();

    assert_eq!(contract.crate_name, "trpg-domain-core");
    assert!(contract.invariants.contains(&"authority_contract_locked"));
    assert!(contract.invariants.contains(&"visibility_label_propagates"));
    assert!(contract.non_goals.contains(&"no_direct_llm_call"));
}
