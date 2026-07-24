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
    "extensions/plugin"
);

use sha2::{Digest, Sha256};
use trpg_domain_core::CommittedFactEvidence;
use trpg_identity::ReplayAuthorization;
use trpg_shared_kernel::{EntityId, TrpgError, Visibility};

/// Trusted host policy input for one plugin invocation. The plugin never sees
/// or constructs this value, so it cannot choose the classification of its
/// own output or substitute a privileged processor for the final audience.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginInvocationContext {
    request_id: EntityId,
    plugin_id: EntityId,
    required_capability: crate::ExtensionCapability,
    input_sha256: String,
    source_fact_ids: Vec<EntityId>,
    campaign_id: EntityId,
    output_visibility: Visibility,
}

impl PluginInvocationContext {
    #[allow(clippy::too_many_arguments)]
    pub fn from_committed_sources(
        request_id: impl Into<String>,
        plugin_id: impl Into<String>,
        required_capability: crate::ExtensionCapability,
        input_json: &str,
        sources: &[CommittedFactEvidence],
        processor_authorization: &ReplayAuthorization,
        target_authorization: &ReplayAuthorization,
        now_unix_ms: u64,
    ) -> Result<Self, TrpgError> {
        if sources.is_empty()
            || required_capability.is_forbidden()
            || processor_authorization.campaign_id() != target_authorization.campaign_id()
            || !matches!(
                serde_json::from_str::<serde_json::Value>(input_json),
                Ok(serde_json::Value::Object(_))
            )
        {
            return Err(TrpgError::VisibilityDenied);
        }
        let mut source_fact_ids = Vec::with_capacity(sources.len());
        let mut source_visibilities = Vec::with_capacity(sources.len());
        for source in sources {
            if source.campaign_id() != processor_authorization.campaign_id()
                || source_fact_ids.contains(source.target_fact_id())
            {
                return Err(TrpgError::PolicyEvidenceUntrusted);
            }
            for authorization in [processor_authorization, target_authorization] {
                let allowed = authorization
                    .can_view(
                        authorization.campaign_id(),
                        source.visibility(),
                        now_unix_ms,
                    )
                    .map_err(|_| TrpgError::AuthenticationRequired)?;
                if !allowed {
                    return Err(TrpgError::VisibilityDenied);
                }
            }
            source_fact_ids.push(source.target_fact_id().clone());
            source_visibilities.push(source.visibility().clone());
        }
        let output_visibility = source_visibilities
            .into_iter()
            .reduce(|current, candidate| current.intersection(&candidate))
            .ok_or(TrpgError::VisibilityDenied)?;
        Ok(Self {
            request_id: EntityId::new(request_id)?,
            plugin_id: EntityId::new(plugin_id)?,
            required_capability,
            input_sha256: sha256(input_json.as_bytes()),
            source_fact_ids,
            campaign_id: processor_authorization.campaign_id().clone(),
            output_visibility,
        })
    }

    pub fn request_id(&self) -> &EntityId {
        &self.request_id
    }

    pub fn source_fact_ids(&self) -> &[EntityId] {
        &self.source_fact_ids
    }

    pub fn campaign_id(&self) -> &EntityId {
        &self.campaign_id
    }

    pub(crate) const fn required_capability(&self) -> crate::ExtensionCapability {
        self.required_capability
    }

    pub(crate) fn input_sha256(&self) -> &str {
        &self.input_sha256
    }

    pub fn derive_output_visibility(&self) -> Result<Visibility, TrpgError> {
        Ok(self.output_visibility.clone())
    }

    pub(crate) fn verify_invocation(
        &self,
        plugin_id: &str,
        capabilities: &[crate::ExtensionCapability],
        input_json: &str,
    ) -> Result<(), TrpgError> {
        if self.plugin_id.as_str() != plugin_id
            || !capabilities.contains(&self.required_capability)
            || sha256(input_json.as_bytes()) != self.input_sha256
        {
            return Err(TrpgError::PolicyEvidenceUntrusted);
        }
        Ok(())
    }
}

fn sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(encoded, "{byte:02x}");
    }
    format!("sha256:{encoded}")
}

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
