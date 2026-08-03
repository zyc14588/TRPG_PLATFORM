use std::fs;
use std::sync::Arc;
use std::time::Duration;

use trpg_agent_runtime::model_provider::{
    Environment, ExecutableModelProvider, LocalProviderNetworkPolicy, ModelChatRequest,
    ModelEmbeddingRequest, ModelMessage, ModelMessageRole, ModelProviderErrorKind,
    ModelProviderRuntimeConfig, ModelReasoningEffort, ModelToolDefinition, ProviderCancellation,
    ProviderCapabilities, ProviderConfig, ProviderType, SecretReference, StructuredOutputRequest,
};
use trpg_agent_runtime::model_provider_local_cloud_impl::HttpModelProvider;
use trpg_agent_runtime::EntityId;
use trpg_security_governance::secret::{KmsClient, KmsSecretResolver, SecretManager};
use trpg_shared_kernel::KernelResult;

struct EnvironmentKms {
    credential: Vec<u8>,
}

impl KmsClient for EnvironmentKms {
    fn decrypt_secret(&self, _secret_id: &str, _version: u64) -> KernelResult<Vec<u8>> {
        Ok(self.credential.clone())
    }
}

type RealProvider = HttpModelProvider<KmsSecretResolver<EnvironmentKms>>;

#[derive(Clone, Copy)]
struct ProviderDefinition<'a> {
    id: &'a str,
    model: &'a str,
    model_sha256: &'a str,
    base_url: &'a str,
    capabilities: ProviderCapabilities,
}

fn required_environment(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} is required"))
}

fn provider_credential(primary: &str, fallback: &str) -> Vec<u8> {
    let path = std::env::var(primary).unwrap_or_else(|_| required_environment(fallback));
    let mut credential = fs::read(path).expect("read real provider credential");
    while credential.last().is_some_and(u8::is_ascii_whitespace) {
        credential.pop();
    }
    assert!(!credential.is_empty(), "real provider credential is empty");
    credential
}

fn provider_type() -> ProviderType {
    match required_environment("TRPG_REAL_PROVIDER_TYPE").as_str() {
        "cloud" => ProviderType::Cloud,
        "ollama" => ProviderType::Ollama,
        "llama_cpp" => ProviderType::LlamaCpp,
        other => panic!("unsupported real provider type: {other}"),
    }
}

fn provider(
    provider_type: ProviderType,
    definition: ProviderDefinition<'_>,
    credential: &[u8],
    root_certificate: Option<&[u8]>,
    policy: &LocalProviderNetworkPolicy,
) -> Result<RealProvider, trpg_agent_runtime::model_provider::ModelProviderError> {
    let credential_reference = SecretReference::kms(format!("{}_credential", definition.id), 1)
        .expect("credential reference");
    let manager = Arc::new(SecretManager::new(KmsSecretResolver::new(EnvironmentKms {
        credential: credential.to_vec(),
    })));
    manager
        .register(&credential_reference)
        .expect("register credential");
    let runtime = ModelProviderRuntimeConfig {
        provider: ProviderConfig {
            provider_id: EntityId::new(definition.id).expect("provider id"),
            provider_type,
            model_id: definition.model.to_owned(),
            model_artifact_sha256: definition.model_sha256.to_owned(),
            base_url: definition.base_url.to_owned(),
            credential: credential_reference,
            environment: Environment::Prod,
        },
        declared_capabilities: definition.capabilities,
        route_authorization_event_id: EntityId::new(format!("{}-route", definition.id))
            .expect("route id"),
        request_timeout: Duration::from_secs(300),
        max_output_tokens: std::num::NonZeroU64::new(512)
            .expect("real-provider output budget is nonzero"),
        cloud_reasoning_effort: (provider_type == ProviderType::Cloud)
            .then_some(ModelReasoningEffort::None),
        development_connect_override: None,
    };
    HttpModelProvider::new_with_root_certificate_and_local_network_policy(
        runtime,
        manager,
        root_certificate,
        policy,
    )
}

fn message(content: &str) -> ModelMessage {
    ModelMessage {
        role: ModelMessageRole::User,
        content: content.to_owned(),
    }
}

include!("real_local_provider_contract_tests/00_negative_contracts.rs");

#[tokio::test]
#[ignore = "requires an authenticated TLS boundary and a real model provider"]
async fn authenticated_real_chat_provider_satisfies_positive_and_negative_contracts() {
    let provider_type = provider_type();
    let chat_url = required_environment("TRPG_REAL_CHAT_PROVIDER_URL");
    let chat_model = required_environment("TRPG_REAL_CHAT_MODEL");
    let chat_sha256 = required_environment("TRPG_REAL_CHAT_MODEL_SHA256");
    let chat_credential = provider_credential(
        "TRPG_REAL_CHAT_PROVIDER_CREDENTIAL_PATH",
        "TRPG_REAL_PROVIDER_CREDENTIAL_PATH",
    );
    let root_certificate = provider_type.is_local().then(|| {
        fs::read(required_environment("TRPG_REAL_PROVIDER_CA_PATH")).expect("read real provider CA")
    });
    let wrong_root_certificate = provider_type.is_local().then(|| {
        fs::read(required_environment("TRPG_REAL_WRONG_PROVIDER_CA_PATH"))
            .expect("read wrong provider CA")
    });
    let policy = LocalProviderNetworkPolicy::parse(
        &std::env::var("TRPG_REAL_PROVIDER_ALLOWLIST").unwrap_or_else(|_| "loopback".to_owned()),
    )
    .expect("real provider allowlist");
    let cancellation = ProviderCancellation::default();
    let definition = ProviderDefinition {
        id: "real-chat",
        model: &chat_model,
        model_sha256: &chat_sha256,
        base_url: &chat_url,
        capabilities: ProviderCapabilities::v1_complete(),
    };
    let chat_provider = provider(
        provider_type,
        definition,
        &chat_credential,
        root_certificate.as_deref(),
        &policy,
    )
    .expect("construct real chat provider");
    let capabilities = chat_provider
        .probe_capabilities(&cancellation)
        .await
        .expect("probe real chat provider")
        .output;
    assert!(capabilities.chat);
    assert!(capabilities.structured_output);
    assert!(capabilities.tool_requests);

    let chat = chat_provider
        .chat(
            &ModelChatRequest {
                messages: vec![message("/no_think\nReply with exactly: LOCAL_OK")],
                structured_output: None,
                tools: Vec::new(),
            },
            &cancellation,
        )
        .await
        .expect("real chat request");
    assert!(!chat.output.content.trim().is_empty());
    assert!(chat.output.usage.output_tokens <= 512);
    println!(
        "provider_usage operation=chat input_tokens={} output_tokens={} max_output_tokens=512",
        chat.output.usage.input_tokens, chat.output.usage.output_tokens
    );

    let structured = chat_provider
        .chat(
            &ModelChatRequest {
                messages: vec![message(
                    "/no_think\nReturn a JSON object whose ok field is true.",
                )],
                structured_output: Some(StructuredOutputRequest {
                    name: "local_contract".to_owned(),
                    schema: serde_json::json!({
                        "type": "object",
                        "properties": {"ok": {"type": "boolean", "const": true}},
                        "required": ["ok"],
                        "additionalProperties": false
                    }),
                }),
                tools: Vec::new(),
            },
            &cancellation,
        )
        .await
        .expect("real structured-output request");
    assert!(structured.output.usage.output_tokens <= 512);
    println!(
        "provider_usage operation=structured_output input_tokens={} output_tokens={} max_output_tokens=512",
        structured.output.usage.input_tokens, structured.output.usage.output_tokens
    );
    assert_eq!(structured.output.structured_output.unwrap()["ok"], true);

    let tool = chat_provider
        .chat(
            &ModelChatRequest {
                messages: vec![message(
                    "/no_think\nCall lookup_clue with clue_id set to clue-1. Do not answer in prose.",
                )],
                structured_output: None,
                tools: vec![ModelToolDefinition {
                    name: "lookup_clue".to_owned(),
                    description: "Look up one governed clue".to_owned(),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "clue_id": {"type": "string", "enum": ["clue-1"]}
                        },
                        "required": ["clue_id"],
                        "additionalProperties": false
                    }),
                }],
            },
            &cancellation,
        )
        .await
        .expect("real tool request");
    assert!(tool.output.usage.output_tokens <= 512);
    println!(
        "provider_usage operation=tool_call input_tokens={} output_tokens={} max_output_tokens=512",
        tool.output.usage.input_tokens, tool.output.usage.output_tokens
    );
    assert!(tool
        .output
        .tool_calls
        .iter()
        .any(|call| call.name == "lookup_clue" && call.arguments["clue_id"] == "clue-1"));

    let wrong_host_url = provider_type
        .is_local()
        .then(|| required_environment("TRPG_REAL_WRONG_HOST_PROVIDER_URL"));
    assert_negative_provider_contracts(
        provider_type,
        definition,
        &chat_credential,
        root_certificate.as_deref(),
        wrong_root_certificate.as_deref(),
        wrong_host_url.as_deref(),
        &policy,
    )
    .await;
}

#[tokio::test]
#[ignore = "requires an authenticated TLS boundary and a real embedding provider"]
async fn authenticated_real_embedding_provider_satisfies_positive_and_negative_contracts() {
    let provider_type = provider_type();
    let embedding_url = required_environment("TRPG_REAL_EMBEDDING_PROVIDER_URL");
    let embedding_model = required_environment("TRPG_REAL_EMBEDDING_MODEL");
    let embedding_sha256 = required_environment("TRPG_REAL_EMBEDDING_MODEL_SHA256");
    let credential = provider_credential(
        "TRPG_REAL_EMBEDDING_PROVIDER_CREDENTIAL_PATH",
        "TRPG_REAL_PROVIDER_CREDENTIAL_PATH",
    );
    let root_certificate = provider_type.is_local().then(|| {
        fs::read(required_environment("TRPG_REAL_PROVIDER_CA_PATH")).expect("read real provider CA")
    });
    let wrong_root_certificate = provider_type.is_local().then(|| {
        fs::read(required_environment("TRPG_REAL_WRONG_PROVIDER_CA_PATH"))
            .expect("read wrong provider CA")
    });
    let policy = LocalProviderNetworkPolicy::parse(
        &std::env::var("TRPG_REAL_PROVIDER_ALLOWLIST").unwrap_or_else(|_| "loopback".to_owned()),
    )
    .expect("real provider allowlist");
    let definition = ProviderDefinition {
        id: "real-embedding",
        model: &embedding_model,
        model_sha256: &embedding_sha256,
        base_url: &embedding_url,
        capabilities: ProviderCapabilities {
            embeddings: true,
            ..ProviderCapabilities::default()
        },
    };
    let embedding_provider = provider(
        provider_type,
        definition,
        &credential,
        root_certificate.as_deref(),
        &policy,
    )
    .expect("construct real embedding provider");
    let cancellation = ProviderCancellation::default();
    let capabilities = embedding_provider
        .probe_capabilities(&cancellation)
        .await
        .expect("probe real embedding provider")
        .output;
    assert!(capabilities.embeddings);
    let embedding = embedding_provider
        .embed(
            &ModelEmbeddingRequest {
                inputs: vec!["Miskatonic University archive".to_owned()],
            },
            &cancellation,
        )
        .await
        .expect("real embedding request");
    assert_eq!(embedding.output.embeddings.len(), 1);
    assert!(embedding.output.embeddings[0].len() >= 32);
    println!(
        "provider_usage operation=embedding vectors={} dimensions={}",
        embedding.output.embeddings.len(),
        embedding.output.embeddings[0].len()
    );

    let wrong_host_url = required_environment("TRPG_REAL_WRONG_HOST_PROVIDER_URL");
    assert_negative_provider_contracts(
        provider_type,
        definition,
        &credential,
        root_certificate.as_deref(),
        wrong_root_certificate.as_deref(),
        Some(&wrong_host_url),
        &policy,
    )
    .await;
}
