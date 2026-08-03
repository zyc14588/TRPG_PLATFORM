
pub fn permission_allows(
    role: PermissionPrincipalRole,
    authority_mode: Option<&trpg_shared_kernel::AuthorityMode>,
    action: SecurityGovernanceAction,
) -> bool {
    use trpg_shared_kernel::AuthorityMode;
    use PermissionPrincipalRole::*;
    use SecurityGovernanceAction::*;

    match (role, action) {
        (ServerOwner, PauseRoom) => true,
        (ServerOwner | CampaignOwner, ManageCampaignMembership) => true,
        (ServerOwner, OverrideDiceRoll) => false,
        (Moderator, MutePlayer) => true,
        (Moderator, ChangeGameDecision) => false,
        (HumanKp, ConfirmAgentDraft) => {
            matches!(authority_mode, Some(mode) if *mode == AuthorityMode::HumanKp)
        }
        (Player, RequestReconsideration) => {
            matches!(authority_mode, Some(mode) if *mode == AuthorityMode::AiKp)
        }
        (Player, OverrideAiDecision) => false,
        (Workflow | RulesEngine | System, WriteOfficialState | RecordAudit) => true,
        (Workflow | System, DeletePersonalData) => true,
        (Workflow | System, ExportPlayerReport | GeneratePartySummary | IndexRagChunk) => true,
        (System, ConnectProvider) => true,
        (Agent | Provider, WriteOfficialState) => false,
        (_, ImportCopyrightedFullText) => false,
        _ => false,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DerivedObject {
    PlayerExport,
    PartySummary,
    RagChunk,
    DebugLog,
    AgentContext,
    AuditLog,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RedactionOutcome {
    Visible,
    Redacted,
    Omitted,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RedactionDecision {
    pub outcome: RedactionOutcome,
    pub result_visibility: VisibilityLabel,
    pub error_code: Option<&'static str>,
}

pub fn evaluate_visibility_derivation(
    source: &Visibility,
    principal: &PrincipalScope,
    target: DerivedObject,
) -> RedactionDecision {
    let sources = [source.clone()];
    let decision = evaluate_derived_visibility(DerivationRequest {
        sources: &sources,
        // This compatibility facade represents the trusted governance worker.
        // The supplied principal remains exclusively the output audience;
        // processor authority cannot make the result player-visible.
        processor: &PrincipalScope::System,
        target_audience: principal,
        target,
    });
    RedactionDecision {
        outcome: decision.outcome,
        result_visibility: decision.result_visibility.label().clone(),
        error_code: decision.error_code,
    }
}

pub fn most_restrictive_visibility(labels: &[VisibilityLabel]) -> VisibilityLabel {
    labels
        .iter()
        .cloned()
        .reduce(|current, candidate| current.conservative_merge(&candidate))
        .unwrap_or(VisibilityLabel::Public)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeploymentEnvironment {
    Development,
    Production,
}

impl DeploymentEnvironment {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Development => "development",
            Self::Production => "production",
        }
    }
}

/// Deployment-owned network boundary for local model providers.
///
/// Loopback remains implicitly available for development and same-network-
/// namespace deployments. Production container or host-gateway endpoints must
/// be named explicitly; accepting a syntactically valid private address is not
/// itself authorization to send model traffic there.
#[derive(Clone, PartialEq, Eq)]
pub struct LocalProviderNetworkPolicy {
    entries: Vec<LocalProviderNetworkEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum LocalProviderNetworkEntry {
    Dns(String),
    Cidr {
        network: std::net::IpAddr,
        prefix: u8,
    },
}

impl LocalProviderNetworkPolicy {
    pub const fn loopback_only() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Parses a comma-separated policy such as
    /// `loopback,dns:ollama-proxy,cidr:172.20.0.0/16`.
    ///
    /// DNS entries are restricted to Compose-style single labels. CIDRs must
    /// be canonical RFC1918, IPv4 loopback, IPv6 unique-local, or IPv6
    /// loopback networks.
    pub fn parse(value: &str) -> KernelResult<Self> {
        let mut entries = Vec::new();
        let value = value.trim();
        if value.is_empty() {
            return Err(TrpgError::InvalidConfiguration(
                "local_provider_allowlist_invalid",
            ));
        }

        for raw_entry in value.split(',') {
            let raw_entry = raw_entry.trim();
            if raw_entry.eq_ignore_ascii_case("loopback") {
                continue;
            }
            if let Some(host) = raw_entry.strip_prefix("dns:") {
                let host = host.trim().to_ascii_lowercase();
                if !safe_private_dns_name(&host) {
                    return Err(TrpgError::InvalidConfiguration(
                        "local_provider_allowlist_invalid",
                    ));
                }
                entries.push(LocalProviderNetworkEntry::Dns(host));
                continue;
            }
            if let Some(cidr) = raw_entry.strip_prefix("cidr:") {
                entries.push(parse_private_cidr(cidr.trim())?);
                continue;
            }
            return Err(TrpgError::InvalidConfiguration(
                "local_provider_allowlist_invalid",
            ));
        }

        entries.sort();
        entries.dedup();
        if entries.len() > 16 {
            return Err(TrpgError::InvalidConfiguration(
                "local_provider_allowlist_invalid",
            ));
        }
        Ok(Self { entries })
    }

    pub fn canonical(&self) -> String {
        let mut values = vec!["loopback".to_owned()];
        values.extend(self.entries.iter().map(|entry| match entry {
            LocalProviderNetworkEntry::Dns(host) => format!("dns:{host}"),
            LocalProviderNetworkEntry::Cidr { network, prefix } => {
                format!("cidr:{network}/{prefix}")
            }
        }));
        values.join(",")
    }

    pub fn permits(&self, endpoint: &url::Url) -> bool {
        let Some(host) = endpoint.host_str() else {
            return false;
        };
        if host.eq_ignore_ascii_case("localhost") {
            return true;
        }
        if let Ok(address) = host.parse::<std::net::IpAddr>() {
            if address.is_loopback() {
                return true;
            }
            return self.entries.iter().any(|entry| match entry {
                LocalProviderNetworkEntry::Cidr { network, prefix } => {
                    address_in_cidr(address, *network, *prefix)
                }
                LocalProviderNetworkEntry::Dns(_) => false,
            });
        }
        self.entries.iter().any(|entry| {
            matches!(entry, LocalProviderNetworkEntry::Dns(allowed) if allowed.eq_ignore_ascii_case(host))
        })
    }

    pub fn endpoint_has_supported_private_shape(endpoint: &url::Url) -> bool {
        local_endpoint_has_supported_private_shape(endpoint)
    }
}

impl Default for LocalProviderNetworkPolicy {
    fn default() -> Self {
        Self::loopback_only()
    }
}

impl std::fmt::Debug for LocalProviderNetworkPolicy {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LocalProviderNetworkPolicy")
            .field("entry_count", &self.entries.len())
            .finish()
    }
}

include!("03_permission_allows/01_local_network_validation.rs");
include!("03_permission_allows/02_provider_boundary_and_content.rs");
