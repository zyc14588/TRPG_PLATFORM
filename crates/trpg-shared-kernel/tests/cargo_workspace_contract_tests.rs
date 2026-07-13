use trpg_shared_kernel::cargo_workspace::{
    append_workspace_validated, validate_workspace, CrateRole, CrateSpec, ValidateWorkspacePayload,
    WorkspaceTopology,
};
use trpg_shared_kernel::shared_kernel::{ActorRole, AuthorityMode, EventStore, TrpgError};

fn valid_topology() -> WorkspaceTopology {
    WorkspaceTopology {
        name: "coc_ai_trpg".to_owned(),
        crates: vec![CrateSpec {
            name: "trpg-shared-kernel".to_owned(),
            path: "crates/trpg-shared-kernel".to_owned(),
            role: CrateRole::SharedKernel,
        }],
    }
}

#[test]
fn cargo_workspace_requires_shared_kernel_member() {
    validate_workspace(&valid_topology()).unwrap();

    let invalid = WorkspaceTopology {
        name: "coc_ai_trpg".to_owned(),
        crates: Vec::new(),
    };

    assert!(matches!(
        validate_workspace(&invalid),
        Err(TrpgError::WorkspaceViolation(_))
    ));
}

#[test]
fn cargo_workspace_appends_events_with_idempotency_and_expected_version() {
    let payload = ValidateWorkspacePayload {
        topology: valid_topology(),
    };
    let command = trpg_test_support::governed_command!(
        payload,
        ActorRole::HumanKeeper,
        AuthorityMode::HumanKp
    );
    let mut store = EventStore::default();

    let event = append_workspace_validated(&mut store, &command).unwrap();
    assert_eq!(event.sequence, 1);
    assert_eq!(event.payload.crate_count, 1);

    let mut duplicate_command = command.clone();
    duplicate_command.expected_version = 1;
    assert_eq!(
        append_workspace_validated(&mut store, &duplicate_command).unwrap_err(),
        TrpgError::DuplicateCommand
    );

    let mut stale_command = command;
    stale_command.idempotency_key = "idem_002".to_owned();
    assert_eq!(
        append_workspace_validated(&mut store, &stale_command).unwrap_err(),
        TrpgError::ExpectedVersionConflict {
            expected: 0,
            actual: 1
        }
    );
}

#[test]
fn cargo_workspace_rejects_source_path_derived_crate_names() {
    let invalid = WorkspaceTopology {
        name: "coc_ai_trpg".to_owned(),
        crates: vec![CrateSpec {
            name: "docs/implementation/shared-kernel".to_owned(),
            path: "crates/trpg-shared-kernel".to_owned(),
            role: CrateRole::SharedKernel,
        }],
    };

    assert!(matches!(
        validate_workspace(&invalid),
        Err(TrpgError::WorkspaceViolation(_))
    ));
}
