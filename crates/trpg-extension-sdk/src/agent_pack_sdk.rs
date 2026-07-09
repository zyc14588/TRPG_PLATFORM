crate::define_extension_sdk_module!(
    AgentPackSdkCommand,
    AgentPackSdkService,
    append_agent_pack_sdk_event,
    "CODEX-0103-12-EXTENSION-SDK-1322493559",
    "agent_pack_sdk",
    "ExtensionAgentPackSdkRecorded",
    crate::ExtensionOperation::AgentPackSdk,
    ["agent_pack_manifest", "agent_gateway_runtime_contract"],
    [
        crate::ExtensionCapability::RegisterAgentPack,
        crate::ExtensionCapability::EmitProposedDecision,
        crate::ExtensionCapability::ReadProjection,
    ],
    "evidence/batches/BATCH-044/agent-pack-sdk.md"
);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentPackManifest {
    pub pack_id: String,
    pub schema_version: String,
    pub orchestrator_boundary: &'static str,
    pub provider_certification_level: u8,
}

impl AgentPackManifest {
    pub fn fixture() -> Self {
        Self {
            pack_id: "coc7_sample_agent_pack".to_owned(),
            schema_version: "agent_pack.v1".to_owned(),
            orchestrator_boundary: "agent_gateway_orchestrator_runtime",
            provider_certification_level: 4,
        }
    }

    pub fn can_run_keeper_orchestrator(&self) -> bool {
        self.orchestrator_boundary == "agent_gateway_orchestrator_runtime"
            && self.provider_certification_level >= 4
    }

    pub fn direct_model_access_allowed(&self) -> bool {
        false
    }
}
