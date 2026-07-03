use crate::shared_kernel::{KernelResult, TrpgError};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceMember {
    pub package_name: String,
    pub path: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceManifest {
    pub resolver: String,
    pub members: Vec<WorkspaceMember>,
}

pub fn validate_workspace_manifest(manifest: &WorkspaceManifest) -> KernelResult<()> {
    if manifest.resolver != "2" {
        return Err(TrpgError::WorkspaceViolation(
            "workspace resolver 2 is required",
        ));
    }

    let shared_kernel_member = manifest.members.iter().find(|member| {
        member.package_name == "trpg-shared-kernel" && member.path == "crates/trpg-shared-kernel"
    });

    if shared_kernel_member.is_none() {
        return Err(TrpgError::WorkspaceViolation(
            "trpg-shared-kernel workspace member is required",
        ));
    }

    for member in &manifest.members {
        let previous_docs_token = format!("docs-{}", "implementation");
        let generated_source_token = format!("generated-from-{}", "source");
        if member.package_name.contains(&previous_docs_token)
            || member.package_name.contains(&generated_source_token)
        {
            return Err(TrpgError::WorkspaceViolation(
                "workspace package names must be current-safe module names",
            ));
        }
    }

    Ok(())
}
