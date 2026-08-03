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
