crate::define_extension_sdk_module!(
    ToolProviderSdkCommand,
    ToolProviderSdkService,
    append_tool_provider_sdk_event,
    "CODEX-0107-12-EXTENSION-SDK-18948e0a9e",
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
    "evidence/batches/BATCH-044/tool-provider-sdk.md"
);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolProviderManifest {
    pub provider_id: String,
    pub tool_schema_version: String,
    pub returns_visibility_labels: bool,
    pub returns_fact_provenance: bool,
}

impl ToolProviderManifest {
    pub fn fixture() -> Self {
        Self {
            provider_id: "coc7_sample_tool_provider".to_owned(),
            tool_schema_version: "tool_schema.v1".to_owned(),
            returns_visibility_labels: true,
            returns_fact_provenance: true,
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
