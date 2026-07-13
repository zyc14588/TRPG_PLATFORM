crate::define_extension_sdk_module!(
    ExtensionCompatibilityMatrixCommand,
    ExtensionCompatibilityMatrixService,
    append_extension_compatibility_matrix_event,
    "extension_compatibility_matrix",
    "ExtensionCompatibilityMatrixRecorded",
    crate::ExtensionOperation::ExtensionCompatibilityMatrix,
    ["sdk_compatibility_report", "extension_manifest"],
    [
        crate::ExtensionCapability::ReadProjection,
        crate::ExtensionCapability::InvokeGrantedTool,
    ],
    "evidence/extensions/extension-compatibility-matrix.md"
);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompatibilityMatrix {
    pub required_ruleset_version: String,
    pub required_tool_schema_version: String,
}

impl CompatibilityMatrix {
    pub fn current() -> Self {
        Self {
            required_ruleset_version: "7e".to_owned(),
            required_tool_schema_version: "tool_schema.v1".to_owned(),
        }
    }

    pub fn evaluate(
        &self,
        report: &crate::SdkCompatibilityReport,
    ) -> crate::ExtensionSdkResult<()> {
        if !report.has_required_fields() {
            return Err(crate::ExtensionSdkError::CompatibilityRejected(
                crate::WireErrorCode::ExtensionCompatibilityFieldsMissing,
            ));
        }

        if report.ruleset_version != self.required_ruleset_version
            || report.tool_schema_version != self.required_tool_schema_version
            || report.compatibility_result != crate::CompatibilityResult::Compatible
        {
            return Err(crate::ExtensionSdkError::CompatibilityRejected(
                crate::WireErrorCode::ExtensionCompatibilityRejected,
            ));
        }

        Ok(())
    }
}

pub fn record_compatible_extension<T>(
    store: &mut crate::ExtensionEventStore,
    authority: &crate::AuthorityContract,
    command: &crate::CommandEnvelope<T>,
    report: crate::SdkCompatibilityReport,
) -> crate::KernelResult<crate::ExtensionEventEnvelope> {
    crate::readme::record_compatibility_report(store, authority, command, contract(), report)
}
