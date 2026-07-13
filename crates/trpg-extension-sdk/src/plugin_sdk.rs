crate::define_extension_sdk_module!(
    PluginSdkCommand,
    PluginSdkService,
    append_plugin_sdk_event,
    "plugin_sdk",
    "ExtensionPluginSdkRecorded",
    crate::ExtensionOperation::PluginSdk,
    ["plugin_manifest", "tool_grant_record", "audit_record"],
    [
        crate::ExtensionCapability::RegisterPlugin,
        crate::ExtensionCapability::InvokeGrantedTool,
        crate::ExtensionCapability::ReadProjection,
    ],
    "evidence/extensions/plugin-sdk.md"
);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginManifest {
    pub plugin_id: String,
    pub requested_capabilities: Vec<crate::ExtensionCapability>,
}

impl PluginManifest {
    pub fn new(
        plugin_id: impl Into<String>,
        requested_capabilities: Vec<crate::ExtensionCapability>,
    ) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            requested_capabilities,
        }
    }

    pub fn register(
        &self,
        grants: &crate::ExtensionCapabilityGrantSet,
    ) -> crate::ExtensionSdkResult<PluginRegistration> {
        for capability in &self.requested_capabilities {
            grants.require(*capability)?;
        }

        Ok(PluginRegistration {
            plugin_id: self.plugin_id.clone(),
            granted_capabilities: grants.granted().to_vec(),
            event_store_write_allowed: false,
            direct_llm_allowed: false,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginRegistration {
    pub plugin_id: String,
    pub granted_capabilities: Vec<crate::ExtensionCapability>,
    pub event_store_write_allowed: bool,
    pub direct_llm_allowed: bool,
}
