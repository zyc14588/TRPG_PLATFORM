struct ModelProviderEnvironment {
    provider_type: ProviderType,
    provider_id: trpg_agent_runtime::EntityId,
    model_id: String,
    model_artifact_sha256: String,
    base_url: String,
    credential: SecretReference,
    route_authorization_event_id: trpg_agent_runtime::EntityId,
    capabilities: ProviderCapabilities,
    request_timeout: Duration,
    max_output_tokens: std::num::NonZeroU64,
    cloud_reasoning_effort:
        Option<trpg_agent_runtime::model_provider::ModelReasoningEffort>,
    local_network_policy: trpg_agent_runtime::model_provider::LocalProviderNetworkPolicy,
}

impl ModelProviderEnvironment {
    fn from_environment() -> Result<Self, String> {
        let provider_type =
            parse_model_provider_type(&required_environment("TRPG_MODEL_PROVIDER_TYPE")?)?;
        let provider_id = trpg_agent_runtime::EntityId::new(required_environment(
            "TRPG_MODEL_PROVIDER_ID",
        )?)
        .map_err(|_| "TRPG_MODEL_PROVIDER_ID_INVALID".to_owned())?;
        let model_id = required_environment("TRPG_MODEL_ID")?;
        let model_artifact_sha256 =
            required_environment("TRPG_MODEL_ARTIFACT_SHA256")?;
        let base_url = required_environment("TRPG_MODEL_PROVIDER_BASE_URL")?;
        let credential_id =
            required_environment("TRPG_MODEL_PROVIDER_CREDENTIAL_SECRET_ID")?;
        let credential_version = required_environment(
            "TRPG_MODEL_PROVIDER_CREDENTIAL_SECRET_VERSION",
        )?
        .parse::<u64>()
        .ok()
        .filter(|version| *version > 0)
        .ok_or_else(|| {
            "TRPG_MODEL_PROVIDER_CREDENTIAL_SECRET_VERSION_INVALID".to_owned()
        })?;
        let credential = SecretReference::mounted(credential_id, credential_version)
            .map_err(|_| "TRPG_MODEL_PROVIDER_CREDENTIAL_REFERENCE_INVALID".to_owned())?;
        let route_authorization_event_id = trpg_agent_runtime::EntityId::new(
            required_environment("TRPG_MODEL_ROUTE_AUTHORIZATION_EVENT_ID")?,
        )
        .map_err(|_| "TRPG_MODEL_ROUTE_AUTHORIZATION_EVENT_ID_INVALID".to_owned())?;
        let capabilities = parse_model_provider_capabilities(&required_environment(
            "TRPG_MODEL_PROVIDER_CAPABILITIES",
        )?)?;
        let request_timeout = required_environment("TRPG_MODEL_PROVIDER_TIMEOUT_MS")?
            .parse::<u64>()
            .ok()
            .filter(|milliseconds| *milliseconds > 0)
            .map(Duration::from_millis)
            .ok_or_else(|| "TRPG_MODEL_PROVIDER_TIMEOUT_MS_INVALID".to_owned())?;
        let max_output_tokens = std::num::NonZeroU64::new(bounded_environment_u64(
            "TRPG_AGENT_JOB_MAX_OUTPUT_TOKENS",
            4_096,
            1,
            100_000,
        )?)
        .ok_or_else(|| "TRPG_AGENT_JOB_MAX_OUTPUT_TOKENS_INVALID".to_owned())?;
        let cloud_reasoning_effort = optional_model_reasoning_effort_from_environment()?;
        let local_network_policy =
            trpg_agent_runtime::model_provider::LocalProviderNetworkPolicy::parse(
                &required_environment("TRPG_LOCAL_PROVIDER_ENDPOINT_ALLOWLIST")?,
            )
            .map_err(|_| "TRPG_LOCAL_PROVIDER_ENDPOINT_ALLOWLIST_INVALID".to_owned())?;
        Ok(Self {
            provider_type,
            provider_id,
            model_id,
            model_artifact_sha256,
            base_url,
            credential,
            route_authorization_event_id,
            capabilities,
            request_timeout,
            max_output_tokens,
            cloud_reasoning_effort,
            local_network_policy,
        })
    }

    fn into_runtime(self) -> ModelProviderRuntimeConfig {
        ModelProviderRuntimeConfig {
            provider: ProviderConfig {
                provider_id: self.provider_id,
                provider_type: self.provider_type,
                model_id: self.model_id,
                model_artifact_sha256: self.model_artifact_sha256,
                base_url: self.base_url,
                credential: self.credential,
                environment: ModelEnvironment::Prod,
            },
            declared_capabilities: self.capabilities,
            route_authorization_event_id: self.route_authorization_event_id,
            request_timeout: self.request_timeout,
            max_output_tokens: self.max_output_tokens,
            cloud_reasoning_effort: self.cloud_reasoning_effort,
            development_connect_override: None,
        }
    }
}

fn model_provider_from_environment(
    secret_manager: Arc<SecretManager<MountedFileSecretResolver>>,
) -> Result<HttpModelProvider<MountedFileSecretResolver>, String> {
    let environment = ModelProviderEnvironment::from_environment()?;
    let provider_ca = optional_file_bytes("TRPG_MODEL_PROVIDER_CA_CERT_PATH")?;
    let local_network_policy = environment.local_network_policy.clone();
    secret_manager
        .register(&environment.credential)
        .map_err(|_| "MODEL_PROVIDER_CREDENTIAL_REGISTRATION_FAILED".to_owned())?;
    HttpModelProvider::new_with_root_certificate_and_local_network_policy(
        environment.into_runtime(),
        secret_manager,
        provider_ca.as_deref(),
        &local_network_policy,
    )
        .map_err(|error| error.code().to_owned())
}

fn optional_model_provider_from_environment(
    secret_manager: Arc<SecretManager<MountedFileSecretResolver>>,
) -> Result<Option<HttpModelProvider<MountedFileSecretResolver>>, String> {
    match std::env::var("TRPG_MODEL_PROVIDER_TYPE") {
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => {
            Err("TRPG_MODEL_PROVIDER_TYPE_INVALID".to_owned())
        }
        Ok(value) if value.trim().is_empty() => {
            Err("TRPG_MODEL_PROVIDER_TYPE_INVALID".to_owned())
        }
        Ok(_) => model_provider_from_environment(secret_manager).map(Some),
    }
}

fn parse_model_provider_type(value: &str) -> Result<ProviderType, String> {
    match value {
        "cloud" => Ok(ProviderType::Cloud),
        "ollama" => Ok(ProviderType::Ollama),
        "llama_cpp" => Ok(ProviderType::LlamaCpp),
        _ => Err("TRPG_MODEL_PROVIDER_TYPE_INVALID".to_owned()),
    }
}

fn optional_model_reasoning_effort_from_environment(
) -> Result<Option<trpg_agent_runtime::model_provider::ModelReasoningEffort>, String> {
    match std::env::var("TRPG_MODEL_PROVIDER_REASONING_EFFORT") {
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => {
            Err("TRPG_MODEL_PROVIDER_REASONING_EFFORT_INVALID".to_owned())
        }
        Ok(value) => parse_model_reasoning_effort(&value).map(Some),
    }
}

fn parse_model_reasoning_effort(
    value: &str,
) -> Result<trpg_agent_runtime::model_provider::ModelReasoningEffort, String> {
    use trpg_agent_runtime::model_provider::ModelReasoningEffort;

    match value {
        "none" => Ok(ModelReasoningEffort::None),
        "low" => Ok(ModelReasoningEffort::Low),
        "medium" => Ok(ModelReasoningEffort::Medium),
        "high" => Ok(ModelReasoningEffort::High),
        "xhigh" => Ok(ModelReasoningEffort::XHigh),
        "max" => Ok(ModelReasoningEffort::Max),
        _ => Err("TRPG_MODEL_PROVIDER_REASONING_EFFORT_INVALID".to_owned()),
    }
}

fn parse_model_provider_capabilities(
    value: &str,
) -> Result<ProviderCapabilities, String> {
    let mut capabilities = ProviderCapabilities::default();
    let mut seen = std::collections::HashSet::new();
    for capability in value.split(',').map(str::trim) {
        if capability.is_empty() || !seen.insert(capability) {
            return Err("TRPG_MODEL_PROVIDER_CAPABILITIES_INVALID".to_owned());
        }
        match capability {
            "chat" => capabilities.chat = true,
            "streaming" => capabilities.streaming = true,
            "structured_output" => capabilities.structured_output = true,
            "tool_requests" => capabilities.tool_requests = true,
            "embeddings" => capabilities.embeddings = true,
            _ => {
                return Err("TRPG_MODEL_PROVIDER_CAPABILITIES_INVALID".to_owned());
            }
        }
    }
    if !capabilities.chat {
        return Err("TRPG_MODEL_PROVIDER_CHAT_CAPABILITY_REQUIRED".to_owned());
    }
    Ok(capabilities)
}

#[cfg(test)]
mod model_provider_construction_tests {
    use super::*;

    #[test]
    fn production_worker_accepts_exactly_the_three_ar08_provider_types() {
        assert_eq!(
            parse_model_provider_type("cloud").unwrap(),
            ProviderType::Cloud
        );
        assert_eq!(
            parse_model_provider_type("ollama").unwrap(),
            ProviderType::Ollama
        );
        assert_eq!(
            parse_model_provider_type("llama_cpp").unwrap(),
            ProviderType::LlamaCpp
        );
        assert!(parse_model_provider_type("codex_cli_oss").is_err());
        assert!(parse_model_provider_type("local_openai_compatible").is_err());
    }

    #[test]
    fn production_worker_reasoning_effort_is_explicit_and_bounded() {
        assert_eq!(parse_model_reasoning_effort("none").unwrap().as_str(), "none");
        assert_eq!(parse_model_reasoning_effort("max").unwrap().as_str(), "max");
        assert!(parse_model_reasoning_effort("").is_err());
        assert!(parse_model_reasoning_effort("minimal").is_err());
        assert!(parse_model_reasoning_effort("ultra").is_err());
    }

    #[test]
    fn production_worker_capability_declaration_is_explicit_and_fail_closed() {
        let capabilities = parse_model_provider_capabilities(
            "chat,streaming,structured_output,tool_requests,embeddings",
        )
        .unwrap();
        assert_eq!(capabilities, ProviderCapabilities::v1_complete());
        assert!(parse_model_provider_capabilities("streaming,embeddings").is_err());
        assert!(parse_model_provider_capabilities("chat,unknown").is_err());
        assert!(parse_model_provider_capabilities("chat,chat").is_err());
    }

    #[test]
    fn production_worker_local_network_policy_is_private_and_explicit() {
        let policy = trpg_agent_runtime::model_provider::LocalProviderNetworkPolicy::parse(
            "dns:ollama-proxy,cidr:172.20.0.0/16",
        )
        .unwrap();
        assert_eq!(
            policy.canonical(),
            "loopback,dns:ollama-proxy,cidr:172.20.0.0/16"
        );
        assert!(trpg_agent_runtime::model_provider::LocalProviderNetworkPolicy::parse(
            "dns:api.openai.com"
        )
        .is_err());
    }
}
