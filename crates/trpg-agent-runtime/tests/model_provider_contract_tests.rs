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
    ModelStreamChunk, ModelStreamSink, ModelToolDefinition, ProviderCancellation,
    ProviderCapabilities, ProviderConfig, ProviderType, SecretReference, StructuredOutputRequest,
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
}

impl Drop for MockModelServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn handle_connection(
    mut socket: TcpStream,
    provider_type: ProviderType,
    model_id: &str,
    behavior: Arc<Mutex<MockBehavior>>,
    requests: Arc<Mutex<Vec<RequestMetadata>>>,
    authorization_seen: Arc<AtomicBool>,
) {
    let Some((head, body)) = read_request(&mut socket).await else {
        return;
    };
    let first_line = head.lines().next().unwrap_or_default();
    let path = first_line
        .split_ascii_whitespace()
        .nth(1)
        .unwrap_or_default()
        .to_owned();
    let authorization_present = head.lines().any(|line| {
        line.to_ascii_lowercase()
            .starts_with("authorization: bearer ")
    });
    authorization_seen.fetch_or(authorization_present, Ordering::Relaxed);
    let streaming = body
        .windows(br#""stream":true"#.len())
        .any(|window| window == br#""stream":true"#);
    let thinking_disabled = serde_json::from_slice::<serde_json::Value>(&body)
        .ok()
        .and_then(|value| value.get("think").and_then(serde_json::Value::as_bool));
    requests.lock().unwrap().push(RequestMetadata {
        path: path.clone(),
        streaming,
        authorization_present,
        thinking_disabled,
    });

    let behavior = *behavior.lock().unwrap();
    let is_probe = path.ends_with("/models") || path == "/api/show";
    let is_chat = path.ends_with("/chat/completions") || path == "/api/chat";

    if is_probe {
        if let MockBehavior::ProbeStatus(status) = behavior {
            write_json_response(&mut socket, status, "{}").await;
            return;
        }
        let capabilities = if behavior == MockBehavior::CapabilitiesWithoutTools {
            r#"{"chat":true,"streaming":true,"structured_output":true,"tool_requests":false,"embeddings":true}"#
        } else {
            r#"{"chat":true,"streaming":true,"structured_output":true,"tool_requests":true,"embeddings":true}"#
        };
        let response = if provider_type == ProviderType::Ollama {
            format!(r#"{{"model_info":{{}},"capabilities":{capabilities}}}"#)
        } else {
            format!(r#"{{"data":[{{"id":"{model_id}","capabilities":{capabilities}}}]}}"#)
        };
        write_json_response(&mut socket, 200, &response).await;
        return;
    }

    if is_chat {
        match behavior {
            MockBehavior::ChatDelay(milliseconds) => {
                tokio::time::sleep(Duration::from_millis(milliseconds)).await;
            }
            MockBehavior::ChatStatus(status) => {
                write_json_response(&mut socket, status, "{}").await;
                return;
            }
            MockBehavior::ChatInvalidJson if !streaming => {
                write_json_response(&mut socket, 200, "{invalid").await;
                return;
            }
            MockBehavior::StreamDisconnect if streaming => {
                let body = if provider_type == ProviderType::Ollama {
                    "{\"message\":{\"content\":\"partial\"},\"done\":false}\n"
                } else {
                    "data: {\"choices\":[{\"delta\":{\"content\":\"partial\"}}]}\n\n"
                };
                write_stream_response(&mut socket, body).await;
                return;
            }
            _ => {}
        }

        if streaming {
            let response = if provider_type == ProviderType::Ollama {
                concat!(
                    "{\"message\":{\"content\":\"The \"},\"done\":false}\n",
                    "{\"message\":{\"content\":\"door opens.\"},\"done\":true}\n"
                )
            } else {
                concat!(
                    "data: {\"choices\":[{\"delta\":{\"content\":\"The \"}}]}\n\n",
                    "data: {\"choices\":[{\"delta\":{\"content\":\"door opens.\"}}]}\n\n",
                    "data: [DONE]\n\n"
                )
            };
            write_stream_response(&mut socket, response).await;
            return;
        }

        let duplicate = behavior == MockBehavior::ChatDuplicateToolCall;
        let response = if provider_type == ProviderType::Ollama {
            let second = if duplicate {
                r#",{"id":"tool-call-1","function":{"name":"search_clue","arguments":{"query":"desk"}}}"#
            } else {
                ""
            };
            format!(
                r#"{{"message":{{"content":"{{\"scene\":\"library\"}}","tool_calls":[{{"id":"tool-call-1","function":{{"name":"search_clue","arguments":{{"query":"clue"}}}}}}{second}]}},"prompt_eval_count":7,"eval_count":5}}"#
            )
        } else {
            let second = if duplicate {
                r#",{"id":"tool-call-1","type":"function","function":{"name":"search_clue","arguments":"{\"query\":\"desk\"}"}}"#
            } else {
                ""
            };
            format!(
                r#"{{"choices":[{{"message":{{"content":"{{\"scene\":\"library\"}}","tool_calls":[{{"id":"tool-call-1","type":"function","function":{{"name":"search_clue","arguments":"{{\"query\":\"clue\"}}"}}}}{second}]}}}}],"usage":{{"prompt_tokens":7,"completion_tokens":5}}}}"#
            )
        };
        write_json_response(&mut socket, 200, &response).await;
        return;
    }

    let response = if provider_type == ProviderType::Ollama {
        r#"{"embeddings":[[0.1,0.2,0.3]],"prompt_eval_count":3}"#
    } else {
        r#"{"data":[{"embedding":[0.1,0.2,0.3]}],"usage":{"prompt_tokens":3}}"#
    };
    write_json_response(&mut socket, 200, response).await;
}

async fn read_request(socket: &mut TcpStream) -> Option<(String, Vec<u8>)> {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    let header_end = loop {
        let read = socket.read(&mut buffer).await.ok()?;
        if read == 0 {
            return None;
        }
        request.extend_from_slice(&buffer[..read]);
        if request.len() > 2 * 1024 * 1024 {
            return None;
        }
        if let Some(position) = request.windows(4).position(|window| window == b"\r\n\r\n") {
            break position + 4;
        }
    };
    let head = String::from_utf8(request[..header_end].to_vec()).ok()?;
    let content_length = head
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .unwrap_or(0);
    while request.len() < header_end.saturating_add(content_length) {
        let read = socket.read(&mut buffer).await.ok()?;
        if read == 0 {
            return None;
        }
        request.extend_from_slice(&buffer[..read]);
    }
    Some((
        head,
        request[header_end..header_end + content_length].to_vec(),
    ))
}

async fn write_json_response(socket: &mut TcpStream, status: u16, body: &str) {
    let reason = match status {
        200 => "OK",
        401 => "Unauthorized",
        429 => "Too Many Requests",
        503 => "Service Unavailable",
        _ => "Error",
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = socket.write_all(response.as_bytes()).await;
    let _ = socket.shutdown().await;
}

async fn write_stream_response(socket: &mut TcpStream, body: &str) {
    let response = format!(
        "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = socket.write_all(response.as_bytes()).await;
    let _ = socket.shutdown().await;
}

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
