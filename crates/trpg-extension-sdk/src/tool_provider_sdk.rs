crate::define_extension_sdk_module!(
    ToolProviderSdkCommand,
    ToolProviderSdkService,
    append_tool_provider_sdk_event,
    "tool_provider_sdk",
    "ExtensionToolProviderSdkRecorded",
    crate::ExtensionOperation::ToolProviderSdk,
    [
        "tool_provider_manifest",
        "tool_result_record",
        "audit_record"
    ],
    [
        crate::ExtensionCapability::RegisterToolProvider,
        crate::ExtensionCapability::InvokeGrantedTool,
        crate::ExtensionCapability::ReadProjection,
    ],
    "extensions/tool-provider"
);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolProviderManifest {
    pub provider_id: String,
    pub tool_schema_version: String,
    pub returns_visibility_labels: bool,
    pub returns_fact_provenance: bool,
}

impl ToolProviderManifest {
    pub fn new(
        provider_id: impl Into<String>,
        tool_schema_version: impl Into<String>,
        returns_visibility_labels: bool,
        returns_fact_provenance: bool,
    ) -> Self {
        Self {
            provider_id: provider_id.into(),
            tool_schema_version: tool_schema_version.into(),
            returns_visibility_labels,
            returns_fact_provenance,
        }
    }

    pub fn is_governed_tool_provider(&self) -> bool {
        self.returns_visibility_labels && self.returns_fact_provenance
    }
}

pub fn authorize_tool_invocation(
    gate: &crate::ExtensionPolicyGate,
) -> crate::ExtensionSdkResult<()> {
    gate.authorize()
}
