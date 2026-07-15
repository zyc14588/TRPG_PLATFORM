use trpg_shared_kernel::cargo_workspace_impl::{
    append_cargo_workspace_impl_reviewed, cargo_workspace_impl_review, cargo_workspace_landing,
    validate_cargo_workspace_landing,
};
use trpg_shared_kernel::rust_cargo_workspace::{WorkspaceManifest, WorkspaceMember};
use trpg_shared_kernel::shared_kernel::{ActorRole, AuthorityMode, EventStore, TrpgError};

fn valid_manifest() -> WorkspaceManifest {
    WorkspaceManifest {
        resolver: "2".to_owned(),
        members: vec![WorkspaceMember {
            package_name: "trpg-shared-kernel".to_owned(),
            path: "crates/trpg-shared-kernel".to_owned(),
        }],
    }
}

#[test]
fn cargo_workspace_impl_accepts_current_safe_manifest() {
    let landing = cargo_workspace_landing(valid_manifest());

    validate_cargo_workspace_landing(&landing).unwrap();
    assert_eq!(
        landing.governance_contract.module_name,
        "cargo_workspace_impl"
    );
}

#[test]
fn cargo_workspace_impl_rejects_invalid_workspace_manifest() {
    let mut manifest = valid_manifest();
    manifest.resolver = "1".to_owned();
    let landing = cargo_workspace_landing(manifest);

    assert!(matches!(
        validate_cargo_workspace_landing(&landing),
        Err(TrpgError::WorkspaceViolation(_))
    ));
}

#[test]
fn cargo_workspace_impl_review_is_recorded_as_a_governed_event() {
    let command = trpg_test_support::governed_command(
        cargo_workspace_impl_review(),
        ActorRole::HumanKeeper,
        AuthorityMode::HumanKp,
    );
    let mut store = EventStore::default();

    let event = append_cargo_workspace_impl_reviewed(&mut store, &command).unwrap();

    assert_eq!(event.payload.module_name, "cargo_workspace_impl");
    assert_eq!(event.payload.reviewed_requirements, 3);
    assert_eq!(event.command_id, command.command_id);
}
