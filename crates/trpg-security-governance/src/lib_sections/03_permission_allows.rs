
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

fn safe_private_dns_name(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 63
        && !host.contains('.')
        && host
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        && host
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        && host
            .as_bytes()
            .last()
            .is_some_and(u8::is_ascii_alphanumeric)
}

fn local_endpoint_has_supported_private_shape(endpoint: &url::Url) -> bool {
    let Some(host) = endpoint.host_str() else {
        return false;
    };
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    match host.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(address)) => address.is_private() || address.is_loopback(),
        Ok(std::net::IpAddr::V6(address)) => address.is_unique_local() || address.is_loopback(),
        Err(_) => safe_private_dns_name(&host.to_ascii_lowercase()),
    }
}

fn parse_private_cidr(value: &str) -> KernelResult<LocalProviderNetworkEntry> {
    let (address, prefix) = value.split_once('/').ok_or(
        TrpgError::InvalidConfiguration("local_provider_allowlist_invalid"),
    )?;
    let address = address.parse::<std::net::IpAddr>().map_err(|_| {
        TrpgError::InvalidConfiguration("local_provider_allowlist_invalid")
    })?;
    let prefix = prefix.parse::<u8>().map_err(|_| {
        TrpgError::InvalidConfiguration("local_provider_allowlist_invalid")
    })?;
    let (canonical_network, final_address, private) = match address {
        std::net::IpAddr::V4(address) if prefix <= 32 => {
            let mask = if prefix == 0 {
                0
            } else {
                u32::MAX << (32 - prefix)
            };
            let network = u32::from(address) & mask;
            let final_address = network | !mask;
            let network_address = std::net::Ipv4Addr::from(network);
            let final_address_value = std::net::Ipv4Addr::from(final_address);
            (
                std::net::IpAddr::V4(network_address),
                std::net::IpAddr::V4(final_address_value),
                (network_address.is_private() && final_address_value.is_private())
                    || (network_address.is_loopback() && final_address_value.is_loopback()),
            )
        }
        std::net::IpAddr::V6(address) if prefix <= 128 => {
            let mask = if prefix == 0 {
                0
            } else {
                u128::MAX << (128 - prefix)
            };
            let network = u128::from(address) & mask;
            let final_address = network | !mask;
            let network_address = std::net::Ipv6Addr::from(network);
            let final_address_value = std::net::Ipv6Addr::from(final_address);
            (
                std::net::IpAddr::V6(network_address),
                std::net::IpAddr::V6(final_address_value),
                (network_address.is_unique_local() && final_address_value.is_unique_local())
                    || (network_address.is_loopback() && final_address_value.is_loopback()),
            )
        }
        _ => {
            return Err(TrpgError::InvalidConfiguration(
                "local_provider_allowlist_invalid",
            ))
        }
    };
    if !private || canonical_network != address || !same_address_family(canonical_network, final_address)
    {
        return Err(TrpgError::InvalidConfiguration(
            "local_provider_allowlist_invalid",
        ));
    }
    Ok(LocalProviderNetworkEntry::Cidr {
        network: canonical_network,
        prefix,
    })
}

fn same_address_family(left: std::net::IpAddr, right: std::net::IpAddr) -> bool {
    matches!(
        (left, right),
        (std::net::IpAddr::V4(_), std::net::IpAddr::V4(_))
            | (std::net::IpAddr::V6(_), std::net::IpAddr::V6(_))
    )
}

fn address_in_cidr(
    address: std::net::IpAddr,
    network: std::net::IpAddr,
    prefix: u8,
) -> bool {
    match (address, network) {
        (std::net::IpAddr::V4(address), std::net::IpAddr::V4(network)) => {
            let mask = if prefix == 0 {
                0
            } else {
                u32::MAX << (32 - prefix)
            };
            u32::from(address) & mask == u32::from(network)
        }
        (std::net::IpAddr::V6(address), std::net::IpAddr::V6(network)) => {
            let mask = if prefix == 0 {
                0
            } else {
                u128::MAX << (128 - prefix)
            };
            u128::from(address) & mask == u128::from(network)
        }
        _ => false,
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ProviderEndpoint {
    provider_type: String,
    base_url: String,
    credential: secret::SecretReference,
    environment: DeploymentEnvironment,
    model_id: String,
    model_artifact_sha256: String,
    local_network_policy: LocalProviderNetworkPolicy,
}

impl ProviderEndpoint {
    pub fn new(
        provider_type: impl Into<String>,
        base_url: impl Into<String>,
        credential: secret::SecretReference,
        environment: DeploymentEnvironment,
        model_id: impl Into<String>,
        model_artifact_sha256: impl Into<String>,
    ) -> KernelResult<Self> {
        let endpoint = Self {
            provider_type: provider_type.into(),
            base_url: base_url.into(),
            credential,
            environment,
            model_id: model_id.into(),
            model_artifact_sha256: model_artifact_sha256.into(),
            local_network_policy: LocalProviderNetworkPolicy::loopback_only(),
        };
        endpoint.validate_model_identity()?;
        Ok(endpoint)
    }

    pub fn provider_type(&self) -> &str {
        &self.provider_type
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn credential(&self) -> &secret::SecretReference {
        &self.credential
    }

    pub const fn environment(&self) -> DeploymentEnvironment {
        self.environment
    }

    pub fn model_id(&self) -> &str {
        &self.model_id
    }

    pub fn model_artifact_sha256(&self) -> &str {
        &self.model_artifact_sha256
    }

    pub fn with_local_network_policy(mut self, policy: LocalProviderNetworkPolicy) -> Self {
        self.local_network_policy = policy;
        self
    }

    pub fn local_network_policy(&self) -> &LocalProviderNetworkPolicy {
        &self.local_network_policy
    }

    fn validate_model_identity(&self) -> KernelResult<()> {
        if self.model_id.trim().is_empty()
            || self.model_id.len() > 256
            || self.model_artifact_sha256.len() != 71
            || !self.model_artifact_sha256.starts_with("sha256:")
            || !self.model_artifact_sha256[7..]
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(TrpgError::InvalidConfiguration(
                "provider_model_identity_invalid",
            ));
        }
        Ok(())
    }
}

impl std::fmt::Debug for ProviderEndpoint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProviderEndpoint")
            .field("provider_type", &self.provider_type)
            .field("base_url", &"[redacted endpoint]")
            .field("credential", &self.credential)
            .field("environment", &self.environment)
            .field("model_id", &self.model_id)
            .field("model_artifact_sha256", &self.model_artifact_sha256)
            .field("local_network_policy", &self.local_network_policy)
            .finish()
    }
}

/// Opaque proof that the exact provider endpoint, model artifact and active
/// secret version were validated together. Callers can persist only the
/// digest; they cannot manufacture an "authenticated" boolean.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderBoundaryAttestation {
    security_snapshot_digest: String,
}

impl ProviderBoundaryAttestation {
    pub fn security_snapshot_digest(&self) -> &str {
        &self.security_snapshot_digest
    }
}

pub fn validate_provider_boundary<R: secret::SecretResolver>(
    endpoint: &ProviderEndpoint,
    secret_manager: &secret::SecretManager<R>,
) -> KernelResult<ProviderBoundaryAttestation> {
    endpoint.validate_model_identity()?;
    if endpoint.environment == DeploymentEnvironment::Production
        && !endpoint.credential.production_eligible()
    {
        return Err(TrpgError::InvalidConfiguration(
            "production_secret_backend_required",
        ));
    }
    let local_provider = match endpoint.provider_type.trim().to_ascii_lowercase().as_str() {
        "cloud" | "cloud-provider" | "openai" | "anthropic" => false,
        "ollama"
        | "llama_cpp"
        | "llama.cpp"
        | "local-model-provider"
        | "local-openai-compatible" => true,
        _ => {
            return Err(TrpgError::InvalidConfiguration(
                "unknown_provider_classification",
            ))
        }
    };
    let base_url = url::Url::parse(&endpoint.base_url)
        .map_err(|_| TrpgError::InvalidConfiguration("provider_endpoint_invalid"))?;
    if base_url.host_str().is_none()
        || !base_url.username().is_empty()
        || base_url.password().is_some()
        || base_url.query().is_some()
        || base_url.fragment().is_some()
    {
        return Err(TrpgError::InvalidConfiguration(
            "provider_endpoint_must_not_contain_credentials",
        ));
    }
    let host_is_loopback = base_url
        .host_str()
        .is_some_and(|host| host.eq_ignore_ascii_case("localhost") || host.parse::<std::net::IpAddr>().is_ok_and(|address| address.is_loopback()));
    if local_provider && !local_endpoint_has_supported_private_shape(&base_url) {
        return Err(TrpgError::InvalidConfiguration(
            "unauthenticated_local_provider_exposed",
        ));
    }
    if local_provider && !endpoint.local_network_policy.permits(&base_url) {
        return Err(TrpgError::InvalidConfiguration(
            "local_provider_endpoint_not_allowlisted",
        ));
    }
    if !local_provider && host_is_loopback {
        return Err(TrpgError::InvalidConfiguration(
            "cloud_provider_endpoint_must_be_remote",
        ));
    }
    if endpoint.environment == DeploymentEnvironment::Production && base_url.scheme() != "https" {
        return Err(TrpgError::InvalidConfiguration(
            "production_provider_https_required",
        ));
    }

    // Resolution is deliberately performed at the configuration boundary.
    // Merely naming a production-capable backend does not prove that the
    // version exists, remains active, or can be decrypted/read.
    let resolved_secret = secret_manager.resolve(&endpoint.credential)?;
    drop(resolved_secret);

    let mut digest = Sha256::new();
    append_provider_snapshot_field(&mut digest, b"trpg-provider-security-snapshot-v2");
    append_provider_snapshot_field(&mut digest, endpoint.environment.as_str().as_bytes());
    append_provider_snapshot_field(
        &mut digest,
        endpoint
            .provider_type
            .trim()
            .to_ascii_lowercase()
            .as_bytes(),
    );
    append_provider_snapshot_field(
        &mut digest,
        endpoint.local_network_policy.canonical().as_bytes(),
    );
    append_provider_snapshot_field(&mut digest, base_url.as_str().as_bytes());
    append_provider_snapshot_field(&mut digest, endpoint.model_id.as_bytes());
    append_provider_snapshot_field(
        &mut digest,
        endpoint
            .model_artifact_sha256
            .to_ascii_lowercase()
            .as_bytes(),
    );
    append_provider_snapshot_field(
        &mut digest,
        match endpoint.credential.backend() {
            secret::SecretBackend::Kms => b"kms",
            secret::SecretBackend::MountedFile => b"mounted_file",
            secret::SecretBackend::DevelopmentMemory => b"development_memory",
        },
    );
    append_provider_snapshot_field(&mut digest, endpoint.credential.secret_id().as_bytes());
    append_provider_snapshot_field(
        &mut digest,
        endpoint.credential.version().to_string().as_bytes(),
    );

    Ok(ProviderBoundaryAttestation {
        security_snapshot_digest: format!("sha256:{:x}", digest.finalize()),
    })
}

fn append_provider_snapshot_field(digest: &mut Sha256, value: &[u8]) {
    digest.update((value.len() as u64).to_be_bytes());
    digest.update(value);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalModelCertificationLevel {
    LocalModelLevel1,
    LocalModelLevel4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalModelCertificationInput {
    pub json_schema_support: bool,
    pub tool_call_support: bool,
    pub visibility_tests_pass: bool,
    pub rules_eval_pass: bool,
    pub latency_ms: u64,
}

pub fn certify_local_model(input: LocalModelCertificationInput) -> LocalModelCertificationLevel {
    if input.json_schema_support
        && input.tool_call_support
        && input.visibility_tests_pass
        && input.rules_eval_pass
        && input.latency_ms <= 2_000
    {
        LocalModelCertificationLevel::LocalModelLevel4
    } else {
        LocalModelCertificationLevel::LocalModelLevel1
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentLicense {
    Original,
    Permissive,
    CopyrightedCommercial,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentUse {
    ShortQuote,
    FullTextImport,
    PlayerExport,
    PrivateReference,
}

pub fn copyright_allows(license: ContentLicense, use_case: ContentUse) -> bool {
    !matches!(
        (license, use_case),
        (
            ContentLicense::CopyrightedCommercial,
            ContentUse::FullTextImport | ContentUse::PlayerExport
        )
    )
}
