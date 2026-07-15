use crate::shared_kernel::{
    validate_command_envelope, CommandEnvelope, EventEnvelope, EventStore, KernelResult, TrpgError,
};

pub const WORKSPACE_VALIDATED_EVENT: &str = "WorkspaceValidated";
pub const CONFIG_CHANGED_SUBJECT: &str = "trpg.foundation.config.changed";
pub const WORKSPACE_VALIDATED_SUBJECT: &str = "trpg.foundation.workspace.validated";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CrateRole {
    SharedKernel,
    Domain,
    Runtime,
    Api,
    Agent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CrateSpec {
    pub name: String,
    pub path: String,
    pub role: CrateRole,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceTopology {
    pub name: String,
    pub crates: Vec<CrateSpec>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidateWorkspacePayload {
    pub topology: WorkspaceTopology,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct WorkspaceValidatedPayload {
    pub workspace_name: String,
    pub crate_count: usize,
}

pub fn validate_workspace(topology: &WorkspaceTopology) -> KernelResult<()> {
    if topology.name.trim().is_empty() {
        return Err(TrpgError::WorkspaceViolation("workspace name is required"));
    }

    let has_shared_kernel = topology.crates.iter().any(|crate_spec| {
        crate_spec.name == "trpg-shared-kernel"
            && crate_spec.path == "crates/trpg-shared-kernel"
            && crate_spec.role == CrateRole::SharedKernel
    });

    if !has_shared_kernel {
        return Err(TrpgError::WorkspaceViolation(
            "trpg-shared-kernel crate must be declared as shared kernel",
        ));
    }

    for crate_spec in &topology.crates {
        if crate_spec.name.contains('/') || crate_spec.name.contains('\\') {
            return Err(TrpgError::WorkspaceViolation(
                "crate name must not be derived from a source path",
            ));
        }
    }

    Ok(())
}

pub fn append_workspace_validated(
    store: &mut EventStore<WorkspaceValidatedPayload>,
    command: &CommandEnvelope<ValidateWorkspacePayload>,
) -> KernelResult<EventEnvelope<WorkspaceValidatedPayload>> {
    validate_command_envelope(command)?;
    validate_workspace(&command.payload.topology)?;

    store.append(
        command,
        WORKSPACE_VALIDATED_EVENT,
        WorkspaceValidatedPayload {
            workspace_name: command.payload.topology.name.clone(),
            crate_count: command.payload.topology.crates.len(),
        },
    )
}
