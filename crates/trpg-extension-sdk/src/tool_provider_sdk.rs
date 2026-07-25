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

use std::fmt;

use serde_json::Value;
use sha2::{Digest, Sha256};
use trpg_security_governance::formal_commit_audit::FormalAuthorization;
use trpg_shared_kernel::{EntityId, FactProvenance, ProvenanceKind, Visibility};

use crate::plugin_host::{PluginOutput, PluginOutputKind};

pub const TOOL_INVOCATION_ACTION: &str = "invoke_granted_tool";
pub const TOOL_INVOCATION_RESOURCE_TYPE: &str = "tool_invocation";
pub const TOOL_INVOCATION_REQUESTED_ROLE: &str = "extension_tool_executor";

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
        // Security metadata is host-owned. A provider that claims to return
        // either field is incompatible with the governed lifecycle.
        !self.returns_visibility_labels && !self.returns_fact_provenance
    }
}

pub fn tool_invocation_resource_id(
    request: &PluginOutput,
    provider_id: &str,
    tool_id: &str,
    tool_schema_version: &str,
) -> crate::ExtensionSdkResult<String> {
    let provider_id = EntityId::new(provider_id)?;
    let tool_id = EntityId::new(tool_id)?;
    if tool_schema_version.trim().is_empty() || tool_schema_version.len() > 128 {
        return Err(crate::ExtensionSdkError::Kernel(
            trpg_shared_kernel::TrpgError::PolicyEvidenceUntrusted,
        ));
    }
    let input = canonical_json(request.payload())?;
    let binding = serde_json::json!({
        "campaign_id": request.campaign_id().as_str(),
        "request_id": request.request_id().as_str(),
        "plugin_id": request.fact_provenance().recorded_by.as_str(),
        "provider_id": provider_id.as_str(),
        "tool_id": tool_id.as_str(),
        "tool_schema_version": tool_schema_version,
        "input_sha256": sha256(&input),
    });
    let digest = sha256(&canonical_json(&binding)?);
    Ok(format!("tool_invocation_{}", &digest["sha256:".len()..]))
}

pub fn authorize_tool_invocation(
    authorization: &FormalAuthorization,
    request: &PluginOutput,
    provider_id: &str,
    tool_id: &str,
    tool_schema_version: &str,
) -> crate::ExtensionSdkResult<String> {
    let expected_resource_id =
        tool_invocation_resource_id(request, provider_id, tool_id, tool_schema_version)?;
    let audit = authorization.canonical_audit();
    if request.kind() != PluginOutputKind::ToolRequest
        || authorization.contract().campaign_id() != request.campaign_id()
        || audit.action != TOOL_INVOCATION_ACTION
        || audit.resource_type != TOOL_INVOCATION_RESOURCE_TYPE
        || audit.resource_id != expected_resource_id
        || audit.requested_role != TOOL_INVOCATION_REQUESTED_ROLE
        || [
            audit.openfga_decision_id.as_str(),
            audit.openfga_policy_revision.as_str(),
            audit.opa_decision_id.as_str(),
            audit.opa_policy_revision.as_str(),
        ]
        .iter()
        .any(|value| value.trim().is_empty())
    {
        return Err(crate::ExtensionSdkError::Kernel(
            trpg_shared_kernel::TrpgError::PolicyEvidenceUntrusted,
        ));
    }
    Ok(expected_resource_id)
}

/// Immutable evidence minted only after a granted tool executor returned a
/// concrete result. Input/output digests bind the receipt to one request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutedToolReceipt {
    request_id: EntityId,
    provider_id: EntityId,
    tool_id: EntityId,
    tool_schema_version: String,
    input_sha256: String,
    output_sha256: String,
    authorization_resource_id: String,
    openfga_decision_id: String,
    opa_decision_id: String,
}

impl ExecutedToolReceipt {
    pub fn request_id(&self) -> &EntityId {
        &self.request_id
    }

    pub fn provider_id(&self) -> &EntityId {
        &self.provider_id
    }

    pub fn tool_id(&self) -> &EntityId {
        &self.tool_id
    }

    pub fn tool_schema_version(&self) -> &str {
        &self.tool_schema_version
    }

    pub fn input_sha256(&self) -> &str {
        &self.input_sha256
    }

    pub fn output_sha256(&self) -> &str {
        &self.output_sha256
    }

    pub fn authorization_resource_id(&self) -> &str {
        &self.authorization_resource_id
    }
}

#[derive(Clone, PartialEq)]
pub struct TrustedToolResult {
    receipt: ExecutedToolReceipt,
    visibility: Visibility,
    fact_provenance: FactProvenance,
    payload: Value,
}

impl fmt::Debug for TrustedToolResult {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TrustedToolResult")
            .field("receipt", &self.receipt)
            .field("visibility", &self.visibility)
            .field("fact_provenance", &self.fact_provenance)
            .field("payload", &"[REDACTED TOOL PAYLOAD]")
            .finish()
    }
}

impl TrustedToolResult {
    pub fn receipt(&self) -> &ExecutedToolReceipt {
        &self.receipt
    }

    pub fn visibility(&self) -> &Visibility {
        &self.visibility
    }

    pub fn fact_provenance(&self) -> &FactProvenance {
        &self.fact_provenance
    }

    pub fn payload(&self) -> &Value {
        &self.payload
    }
}

/// Executes a plugin-originated request behind the host policy gate and mints
/// ToolResult provenance only after the executor succeeds. A plugin can
/// create neither `ExecutedToolReceipt` nor `TrustedToolResult` directly.
pub fn execute_granted_tool(
    authorization: &FormalAuthorization,
    request: &PluginOutput,
    provider_id: impl Into<String>,
    tool_id: impl Into<String>,
    tool_schema_version: impl Into<String>,
    executor: impl FnOnce(&Value) -> crate::ExtensionSdkResult<Value>,
) -> crate::ExtensionSdkResult<TrustedToolResult> {
    let provider_id = EntityId::new(provider_id.into())?;
    let tool_id = EntityId::new(tool_id.into())?;
    let tool_schema_version = tool_schema_version.into();
    let authorization_resource_id = authorize_tool_invocation(
        authorization,
        request,
        provider_id.as_str(),
        tool_id.as_str(),
        &tool_schema_version,
    )?;
    let input = canonical_json(request.payload())?;
    let payload = executor(request.payload())?;
    if !matches!(payload, Value::Object(_)) {
        return Err(crate::ExtensionSdkError::Kernel(
            trpg_shared_kernel::TrpgError::PolicyEvidenceUntrusted,
        ));
    }
    let output = canonical_json(&payload)?;
    let receipt = ExecutedToolReceipt {
        request_id: request.request_id().clone(),
        provider_id: provider_id.clone(),
        tool_id,
        tool_schema_version,
        input_sha256: sha256(&input),
        output_sha256: sha256(&output),
        authorization_resource_id,
        openfga_decision_id: authorization.canonical_audit().openfga_decision_id.clone(),
        opa_decision_id: authorization.canonical_audit().opa_decision_id.clone(),
    };
    let provenance_reference = format!("tool_result_{}", &receipt.output_sha256[7..]);
    let fact_provenance = FactProvenance::new(
        ProvenanceKind::ToolResult,
        provenance_reference,
        provider_id.to_string(),
    )?;
    Ok(TrustedToolResult {
        receipt,
        visibility: request.visibility().clone(),
        fact_provenance,
        payload,
    })
}

fn canonical_json(value: &Value) -> crate::ExtensionSdkResult<Vec<u8>> {
    serde_json::to_vec(value).map_err(|_| {
        crate::ExtensionSdkError::Kernel(trpg_shared_kernel::TrpgError::PolicyEvidenceUntrusted)
    })
}

fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}
