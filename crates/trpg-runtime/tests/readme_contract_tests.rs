use trpg_runtime::readme;

#[test]
fn readme_contract_records_runtime_governance_invariants() {
    let contract = readme::runtime_readme_contract();

    assert_eq!(
        readme::PROMPT_ID,
        "CODEX-0358-03-RUNTIME-ORCHESTRATION-5626fcbd5c"
    );
    assert!(
        readme::SUPPLEMENTAL_PROMPT_IDS.contains(&"CODEX-0374-03-RUNTIME-ORCHESTRATION-989f2ac19c")
    );
    assert_eq!(contract.crate_name, "trpg-runtime");
    assert_eq!(contract.module_prefix, "runtime_orchestration");
    assert!(contract
        .invariants
        .iter()
        .any(|item| item.contains("Command -> Workflow -> Decision -> Event Store")));
    assert!(contract.non_goals.contains(&"direct LLM provider calls"));
    assert!(contract.non_goals.contains(&"agent direct database writes"));
}
