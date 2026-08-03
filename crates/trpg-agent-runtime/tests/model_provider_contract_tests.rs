use async_trait::async_trait;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use trpg_agent_runtime::model_provider::{
    Environment, ExecutableModelProvider, ModelChatRequest, ModelEmbeddingRequest, ModelMessage,
    ModelMessageRole, ModelOperation, ModelProviderErrorKind, ModelProviderRuntimeConfig,
    ModelReasoningEffort, ModelStreamChunk, ModelStreamSink, ModelToolDefinition,
    ProviderCancellation, ProviderCapabilities, ProviderConfig, ProviderType, SecretReference,
    StructuredOutputRequest,
};
use trpg_agent_runtime::model_provider_local_cloud::ExplicitModelProviderRouter;
use trpg_agent_runtime::model_provider_local_cloud_impl::HttpModelProvider;
use trpg_agent_runtime::EntityId;
use trpg_security_governance::secret::{KmsClient, KmsSecretResolver, SecretManager};

const API_KEY_CANARY: &str = "sk-ar08-provider-secret-canary";
const PRIVATE_PROMPT_CANARY: &str = "keeper-only-ar08-private-prompt-canary";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MockBehavior {
    Normal,
    ChatInvalidJson,
    ChatDuplicateToolCall,
    ChatRejectJsonSchema,
    ChatRejectJsonSchemaWithInvalidFallback,
    ChatDelay(u64),
    ChatStatus(u16),
    StreamDisconnect,
    ProbeStatus(u16),
    CapabilitiesWithoutTools,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RequestMetadata {
    path: String,
    streaming: bool,
    authorization_present: bool,
    thinking_disabled: Option<bool>,
    max_output_tokens: Option<u64>,
    reasoning_effort: Option<String>,
    structured_output_format: Option<String>,
}

struct MockModelServer {
    address: SocketAddr,
    behavior: Arc<Mutex<MockBehavior>>,
    requests: Arc<Mutex<Vec<RequestMetadata>>>,
    authorization_seen: Arc<AtomicBool>,
    task: tokio::task::JoinHandle<()>,
}

impl MockModelServer {
    async fn spawn(provider_type: ProviderType, model_id: &str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let behavior = Arc::new(Mutex::new(MockBehavior::Normal));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let authorization_seen = Arc::new(AtomicBool::new(false));
        let task_behavior = Arc::clone(&behavior);
        let task_requests = Arc::clone(&requests);
        let task_authorization = Arc::clone(&authorization_seen);
        let model_id = model_id.to_owned();
        let task = tokio::spawn(async move {
            while let Ok((socket, _peer)) = listener.accept().await {
                let behavior = Arc::clone(&task_behavior);
                let requests = Arc::clone(&task_requests);
                let authorization = Arc::clone(&task_authorization);
                let model_id = model_id.clone();
                tokio::spawn(async move {
                    handle_connection(
                        socket,
                        provider_type,
                        &model_id,
                        behavior,
                        requests,
                        authorization,
                    )
                    .await;
                });
            }
        });
        Self {
            address,
            behavior,
            requests,
            authorization_seen,
            task,
        }
    }

    fn set_behavior(&self, behavior: MockBehavior) {
        *self.behavior.lock().unwrap() = behavior;
    }

    fn request_count(&self, suffix: &str) -> usize {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .filter(|request| request.path.ends_with(suffix))
            .count()
    }

    fn total_requests(&self) -> usize {
        self.requests.lock().unwrap().len()
    }

    fn chat_thinking_modes(&self) -> Vec<Option<bool>> {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .filter(|request| {
                request.path.ends_with("/chat/completions") || request.path == "/api/chat"
            })
            .map(|request| request.thinking_disabled)
            .collect()
    }

    fn chat_output_budgets(&self) -> Vec<Option<u64>> {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .filter(|request| {
                request.path.ends_with("/chat/completions") || request.path == "/api/chat"
            })
            .map(|request| request.max_output_tokens)
            .collect()
    }

    fn chat_reasoning_efforts(&self) -> Vec<Option<String>> {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .filter(|request| {
                request.path.ends_with("/chat/completions") || request.path == "/api/chat"
            })
            .map(|request| request.reasoning_effort.clone())
            .collect()
    }

    fn chat_structured_output_formats(&self) -> Vec<Option<String>> {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .filter(|request| {
                request.path.ends_with("/chat/completions") || request.path == "/api/chat"
            })
            .map(|request| request.structured_output_format.clone())
            .collect()
    }
}

impl Drop for MockModelServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

include!("model_provider_contract_tests/00_mock_server_io.rs");

struct TestKms;

impl KmsClient for TestKms {
    fn decrypt_secret(
        &self,
        _secret_id: &str,
        _version: u64,
    ) -> trpg_shared_kernel::KernelResult<Vec<u8>> {
        Ok(API_KEY_CANARY.as_bytes().to_vec())
    }
}

type TestProvider = HttpModelProvider<KmsSecretResolver<TestKms>>;

fn make_provider(
    provider_type: ProviderType,
    server: &MockModelServer,
    capabilities: ProviderCapabilities,
    timeout: Duration,
) -> Arc<TestProvider> {
    let provider_name = provider_type.route_name();
    let model_id = format!("{provider_name}-model");
    let base_url = match provider_type {
        ProviderType::Cloud => format!("http://cloud-provider.test:{}/v1", server.address.port()),
        ProviderType::Ollama => format!("http://127.0.0.1:{}", server.address.port()),
        ProviderType::LlamaCpp => format!("http://127.0.0.1:{}/v1", server.address.port()),
        ProviderType::LocalOpenAiCompatible => unreachable!(),
    };
    let credential = SecretReference::kms(format!("{provider_name}_credential"), 1).unwrap();
    let manager = Arc::new(SecretManager::new(KmsSecretResolver::new(TestKms)));
    manager.register(&credential).unwrap();
    let runtime = ModelProviderRuntimeConfig {
        provider: ProviderConfig {
            provider_id: EntityId::new(format!("{provider_name}-provider")).unwrap(),
            provider_type,
            model_id,
            model_artifact_sha256: format!("sha256:{}", "a".repeat(64)),
            base_url,
            credential,
            environment: Environment::Dev,
        },
        declared_capabilities: capabilities,
        route_authorization_event_id: EntityId::new(format!("{provider_name}-route-authorized"))
            .unwrap(),
        request_timeout: timeout,
        max_output_tokens: std::num::NonZeroU64::new(256).expect("test output budget is nonzero"),
        cloud_reasoning_effort: (provider_type == ProviderType::Cloud)
            .then_some(ModelReasoningEffort::None),
        development_connect_override: (provider_type == ProviderType::Cloud)
            .then_some(server.address),
    };
    Arc::new(HttpModelProvider::new(runtime, manager).unwrap())
}

fn full_chat_request() -> ModelChatRequest {
    ModelChatRequest {
        messages: vec![ModelMessage {
            role: ModelMessageRole::User,
            content: PRIVATE_PROMPT_CANARY.to_owned(),
        }],
        structured_output: Some(StructuredOutputRequest {
            name: "scene_result".to_owned(),
            schema: serde_json::json!({
                "type": "object",
                "properties": {"scene": {"type": "string"}},
                "required": ["scene"],
                "additionalProperties": false,
            }),
        }),
        tools: vec![ModelToolDefinition {
            name: "search_clue".to_owned(),
            description: "Search a governed clue index".to_owned(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {"query": {"type": "string"}},
                "required": ["query"],
                "additionalProperties": false,
            }),
        }],
    }
}

fn plain_chat_request() -> ModelChatRequest {
    ModelChatRequest {
        messages: vec![ModelMessage {
            role: ModelMessageRole::User,
            content: PRIVATE_PROMPT_CANARY.to_owned(),
        }],
        structured_output: None,
        tools: Vec::new(),
    }
}

include!("model_provider_contract_tests/01_common_contract_and_failures.rs");
include!("model_provider_contract_tests/02_routing_and_redaction.rs");
