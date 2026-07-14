crate::define_extension_sdk_module!(
    Adr0008PluginBoundariesCommand,
    Adr0008PluginBoundariesService,
    append_adr_0008_plugin_boundaries_event,
    "adr_0008_plugin_boundaries",
    "ExtensionAdr0008PluginBoundariesRecorded",
    crate::ExtensionOperation::Adr0008PluginBoundaries,
    ["plugin_boundary_policy", "extension_audit_record"],
    [
        crate::ExtensionCapability::RegisterPlugin,
        crate::ExtensionCapability::ReadProjection,
    ],
    "extensions/plugin-boundary-policy"
);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginBoundaryPolicy {
    forbidden: Vec<crate::ExtensionCapability>,
}

impl PluginBoundaryPolicy {
    pub fn current() -> Self {
        Self {
            forbidden: crate::FORBIDDEN_CAPABILITIES.to_vec(),
        }
    }

    pub fn validate(
        &self,
        capability: crate::ExtensionCapability,
    ) -> crate::ExtensionSdkResult<()> {
        if self.forbidden.contains(&capability) {
            Err(crate::ExtensionSdkError::ForbiddenCapability(
                capability.denial_code(),
            ))
        } else {
            Ok(())
        }
    }
}
