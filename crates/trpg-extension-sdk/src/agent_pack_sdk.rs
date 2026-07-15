crate::define_extension_sdk_module!(
    AgentPackSdkCommand,
    AgentPackSdkService,
    append_agent_pack_sdk_event,
    "agent_pack_sdk",
    "ExtensionAgentPackSdkRecorded",
    crate::ExtensionOperation::AgentPackSdk,
    ["agent_pack_manifest", "agent_gateway_runtime_contract"],
    [
        crate::ExtensionCapability::RegisterAgentPack,
        crate::ExtensionCapability::EmitProposedDecision,
        crate::ExtensionCapability::ReadProjection,
    ],
    "extensions/agent-pack"
);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentPackManifest {
    pub pack_id: String,
    pub schema_version: String,
    pub orchestrator_boundary: &'static str,
    pub provider_certification_level: u8,
}

impl AgentPackManifest {
    pub fn new(
        pack_id: impl Into<String>,
        schema_version: impl Into<String>,
        provider_certification_level: u8,
    ) -> Self {
        Self {
            pack_id: pack_id.into(),
            schema_version: schema_version.into(),
            orchestrator_boundary: "agent_gateway_orchestrator_runtime",
            provider_certification_level,
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
