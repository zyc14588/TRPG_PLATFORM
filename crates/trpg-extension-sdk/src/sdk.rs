crate::define_extension_sdk_module!(
    SdkCommand,
    SdkService,
    append_sdk_event,
    "CODEX-0953-12-EXTENSION-SDK-7588c965bd",
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
    "evidence/batches/BATCH-044/sdk.md"
);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtensionSdkManifest {
    pub sdk_version: String,
    pub contract_count: usize,
}

impl ExtensionSdkManifest {
    pub fn current() -> Self {
        Self {
            sdk_version: "s12.batch044.v1".to_owned(),
            contract_count: crate::all_batch_044_contracts().len(),
        }
    }

    pub fn covers_batch_044(&self) -> bool {
        self.contract_count == 8
    }
}
