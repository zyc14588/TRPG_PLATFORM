use trpg_shared_kernel::{
    CommandEnvelope, EventEnvelope, EventStore, FormalWritePath, KernelResult, TrpgError,
};

pub const PLUGIN_TOOL_GRANT_REGISTERED_EVENT: &str =
    "platform.plugin_sdk_impl.tool_grant_registered";
pub const PLUGIN_SDK_IMPL_METRIC_MODULE: &str = "plugin_sdk_impl";
pub const PLUGIN_SDK_IMPL_REQUIRED_METRICS: &[&str] = &[
    "trpg_command_total",
    "trpg_event_append_latency_ms",
    "trpg_policy_deny_total",
    "trpg_projection_lag_events",
    "trpg_visibility_redaction_total",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegisterPluginToolGrant {
    pub plugin_id: String,
    pub tool_name: String,
    pub granted_write_path: FormalWritePath,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub enum PluginSdkEvent {
    PluginToolGrantRegistered {
        plugin_id: String,
        tool_name: String,
        granted_write_path: FormalWritePath,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PluginSdkError {
    PluginIdRequired,
    ToolNameRequired,
    DirectWriteGrantDenied,
}

impl From<PluginSdkError> for TrpgError {
    fn from(error: PluginSdkError) -> Self {
        match error {
            PluginSdkError::PluginIdRequired => {
                TrpgError::InvalidConfiguration("plugin_id_required")
            }
            PluginSdkError::ToolNameRequired => {
                TrpgError::InvalidConfiguration("tool_name_required")
            }
            PluginSdkError::DirectWriteGrantDenied => TrpgError::PolicyDenied,
        }
    }
}

pub type PluginSdkEventEnvelope = EventEnvelope<PluginSdkEvent>;
pub type PluginSdkRepository = EventStore<PluginSdkEvent>;

pub struct PluginSdkService;

impl PluginSdkService {
    pub fn register_plugin_tool_grant(
        repository: &mut PluginSdkRepository,
        command: &CommandEnvelope<RegisterPluginToolGrant>,
    ) -> KernelResult<PluginSdkEventEnvelope> {
        if command.payload.plugin_id.trim().is_empty() {
            return Err(PluginSdkError::PluginIdRequired.into());
        }
        if command.payload.tool_name.trim().is_empty() {
            return Err(PluginSdkError::ToolNameRequired.into());
        }
        if matches!(
            command.payload.granted_write_path,
            FormalWritePath::DirectAgent | FormalWritePath::DirectBusiness
        ) {
            return Err(PluginSdkError::DirectWriteGrantDenied.into());
        }

        repository.append(
            command,
            PLUGIN_TOOL_GRANT_REGISTERED_EVENT,
            PluginSdkEvent::PluginToolGrantRegistered {
                plugin_id: command.payload.plugin_id.clone(),
                tool_name: command.payload.tool_name.clone(),
                granted_write_path: command.payload.granted_write_path.clone(),
            },
        )
    }
}

pub fn register_plugin_tool_grant(
    repository: &mut PluginSdkRepository,
    command: &CommandEnvelope<RegisterPluginToolGrant>,
) -> KernelResult<PluginSdkEventEnvelope> {
    PluginSdkService::register_plugin_tool_grant(repository, command)
}
