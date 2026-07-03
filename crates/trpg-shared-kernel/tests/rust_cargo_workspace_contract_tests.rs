use trpg_shared_kernel::rust_cargo_workspace::{
    validate_workspace_manifest, WorkspaceManifest, WorkspaceMember,
};
use trpg_shared_kernel::TrpgError;

#[test]
fn rust_cargo_workspace_accepts_current_safe_manifest() {
    let manifest = WorkspaceManifest {
        resolver: "2".to_owned(),
        members: vec![WorkspaceMember {
            package_name: "trpg-shared-kernel".to_owned(),
            path: "crates/trpg-shared-kernel".to_owned(),
        }],
    };

    validate_workspace_manifest(&manifest).unwrap();
}

#[test]
fn rust_cargo_workspace_requires_resolver_two() {
    let manifest = WorkspaceManifest {
        resolver: "1".to_owned(),
        members: vec![WorkspaceMember {
            package_name: "trpg-shared-kernel".to_owned(),
            path: "crates/trpg-shared-kernel".to_owned(),
        }],
    };

    assert!(matches!(
        validate_workspace_manifest(&manifest),
        Err(TrpgError::WorkspaceViolation(_))
    ));
}

#[test]
fn rust_cargo_workspace_rejects_previous_source_path_package_names() {
    let previous_docs_token = format!("docs-{}", "implementation");

    let manifest = WorkspaceManifest {
        resolver: "2".to_owned(),
        members: vec![WorkspaceMember {
            package_name: format!("{previous_docs_token}-01-foundation"),
            path: "crates/trpg-shared-kernel".to_owned(),
        }],
    };

    assert!(matches!(
        validate_workspace_manifest(&manifest),
        Err(TrpgError::WorkspaceViolation(_))
    ));
}
