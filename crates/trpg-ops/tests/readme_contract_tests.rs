mod common;

use std::collections::BTreeSet;

use trpg_ops::readme::{append_readme_event, contract};
use trpg_ops::{
    ops_runbook_contracts, upgrade_runbook_contracts, OpsRunbookCommand, OpsRunbookOperation,
};

#[test]
fn readme_records_governed_event() {
    common::assert_runbook_contract(
        contract(),
        OpsRunbookCommand::record(
            OpsRunbookOperation::ReadmeRecord,
            "readme governance",
            "docs/codex/11-ops-migration/README.md",
        ),
        append_readme_event,
    );
}

#[test]
fn batch_042_primary_contracts_are_unique_and_current_safe() {
    let contracts = ops_runbook_contracts();
    assert_eq!(contracts.len(), 9);

    let mut modules = BTreeSet::new();
    for contract in contracts {
        assert!(contract.uses_current_safe_names());
        assert!(modules.insert(contract.module_name));
        assert!(!contract.module_name.contains("generated"));
        assert!(!contract.module_name.contains("source"));
    }
}

#[test]
fn batch_043_primary_contracts_are_unique_and_current_safe() {
    let contracts = upgrade_runbook_contracts();
    assert_eq!(contracts.len(), 2);

    let mut modules = BTreeSet::new();
    for contract in contracts {
        assert!(contract.uses_current_safe_names());
        assert!(modules.insert(contract.module_name));
        assert!(!contract.module_name.contains("generated"));
        assert!(!contract.module_name.contains("source"));
    }
}
