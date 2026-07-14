crate::define_extension_sdk_module!(
    SdkCommand,
    SdkService,
    append_sdk_event,
    "sdk",
    "ExtensionSdkRecorded",
    crate::ExtensionOperation::Sdk,
    [
        "extension_sdk_contract_registry",
        "sdk_compatibility_report"
    ],
    [
        crate::ExtensionCapability::RegisterPlugin,
        crate::ExtensionCapability::RegisterAgentPack,
        crate::ExtensionCapability::RegisterRulesetPack,
        crate::ExtensionCapability::RegisterToolProvider,
        crate::ExtensionCapability::ReadProjection,
    ],
    "extensions/sdk"
);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtensionSdkManifest {
    pub sdk_version: String,
    pub contract_count: usize,
}

impl ExtensionSdkManifest {
    pub fn current() -> Self {
        Self {
            sdk_version: "extension-sdk.v1".to_owned(),
            contract_count: crate::extension_contracts().len(),
        }
    }

    pub fn supports_current_contract_set(&self) -> bool {
        self.contract_count == 8
    }
}
