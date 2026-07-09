mod common;

use trpg_ops::upgrade_rollback_impl::{
    append_upgrade_rollback_impl_event, contract, UpgradeRollbackImplCommand,
    UpgradeRollbackImplExternalContract, UpgradeRollbackImplPolicyGate, UpgradeRollbackImplService,
    EVENT_STORE_APPEND_BOUNDARY, SQLX_TRANSACTION_BOUNDARY,
};
use trpg_ops::{OpsEventStore, TrpgError, Visibility, VisibilityLabel};

#[test]
fn upgrade_rollback_impl_records_governed_event() {
    common::assert_runbook_contract(
        contract(),
        UpgradeRollbackImplCommand::record("upgrade rollback implementation drill"),
        append_upgrade_rollback_impl_event,
    );
}

#[test]
fn upgrade_rollback_impl_emits_primary_contract_evidence() {
    let authority = common::authority_contract();
    let mut store = OpsEventStore::default();
    let command = common::governed_command(
        UpgradeRollbackImplCommand::record("upgrade rollback implementation drill"),
        0,
        "idem_upgrade_rollback_impl_contract",
        Visibility::new(VisibilityLabel::SystemOnly),
    );

    let execution = UpgradeRollbackImplService::default()
        .execute(&mut store, &authority, &command)
        .expect("policy-approved command records evidence");

    assert_eq!(execution.event.event_type, contract().event_type);
    assert_eq!(
        execution.transaction.sqlx_boundary,
        SQLX_TRANSACTION_BOUNDARY
    );
    assert_eq!(
        execution.transaction.event_store_boundary,
        EVENT_STORE_APPEND_BOUNDARY
    );
    assert_eq!(execution.transaction.expected_version, 0);
    assert_eq!(execution.transaction.event_sequence, 1);
    assert!(execution.external_contract.uses_current_safe_names());
    assert_eq!(
        execution.observability.correlation_id,
        command.correlation_id.as_str()
    );
    assert_eq!(
        execution.observability.causation_id,
        command.causation_id.as_str()
    );
}

#[test]
fn upgrade_rollback_impl_policy_gate_denies_tool_openfga_and_opa_failures() {
    for gate in [
        UpgradeRollbackImplPolicyGate::deny_tool_permission(),
        UpgradeRollbackImplPolicyGate::deny_openfga(),
        UpgradeRollbackImplPolicyGate::deny_opa(),
    ] {
        let authority = common::authority_contract();
        let mut store = OpsEventStore::default();
        let command = common::governed_command(
            UpgradeRollbackImplCommand::record("upgrade rollback implementation denied"),
            0,
            "idem_upgrade_rollback_impl_denied",
            Visibility::new(VisibilityLabel::SystemOnly),
        );

        let error = UpgradeRollbackImplService::new(gate)
            .execute(&mut store, &authority, &command)
            .expect_err("policy gate failure blocks event append");

        assert_eq!(error, TrpgError::PolicyDenied);
        assert!(store.events().is_empty());
    }
}

#[test]
fn upgrade_rollback_impl_redacts_restricted_visibility() {
    common::assert_visibility_redaction(
        UpgradeRollbackImplCommand::record("upgrade rollback implementation visibility"),
        append_upgrade_rollback_impl_event,
    );
}

#[test]
fn upgrade_rollback_impl_external_contract_is_current_safe() {
    assert!(UpgradeRollbackImplExternalContract::current().uses_current_safe_names());
}
