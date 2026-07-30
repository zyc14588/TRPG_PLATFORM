#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelOperation {
    CapabilityProbe,
    Chat,
    StreamingChat,
    Embedding,
}

impl ModelOperation {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CapabilityProbe => "capability_probe",
            Self::Chat => "chat",
            Self::StreamingChat => "streaming_chat",
            Self::Embedding => "embedding",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderCapabilities {
    pub chat: bool,
    pub streaming: bool,
    pub structured_output: bool,
    pub tool_requests: bool,
    pub embeddings: bool,
}

impl ProviderCapabilities {
    pub const fn v1_complete() -> Self {
        Self {
            chat: true,
            streaming: true,
            structured_output: true,
            tool_requests: true,
            embeddings: true,
        }
    }

    pub const fn supports(self, capability: RequiredProviderCapability) -> bool {
        match capability {
            RequiredProviderCapability::Chat => self.chat,
            RequiredProviderCapability::Streaming => self.streaming,
            RequiredProviderCapability::StructuredOutput => self.structured_output,
            RequiredProviderCapability::ToolRequests => self.tool_requests,
            RequiredProviderCapability::Embeddings => self.embeddings,
        }
    }

    pub const fn intersection(self, other: Self) -> Self {
        Self {
            chat: self.chat && other.chat,
            streaming: self.streaming && other.streaming,
            structured_output: self.structured_output && other.structured_output,
            tool_requests: self.tool_requests && other.tool_requests,
            embeddings: self.embeddings && other.embeddings,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequiredProviderCapability {
    Chat,
    Streaming,
    StructuredOutput,
    ToolRequests,
    Embeddings,
}

impl RequiredProviderCapability {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::Streaming => "streaming",
            Self::StructuredOutput => "structured_output",
            Self::ToolRequests => "tool_requests",
            Self::Embeddings => "embeddings",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelMessageRole {
    System,
    User,
    Assistant,
    Tool,
}

impl ModelMessageRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::Tool => "tool",
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelMessage {
    pub role: ModelMessageRole,
    pub content: String,
}

impl std::fmt::Debug for ModelMessage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ModelMessage")
            .field("role", &self.role)
            .field("content", &"[redacted model context]")
            .finish()
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct StructuredOutputRequest {
    pub name: String,
    pub schema: serde_json::Value,
}

impl std::fmt::Debug for StructuredOutputRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("StructuredOutputRequest")
            .field("name", &self.name)
            .field("schema", &"[redacted schema]")
            .finish()
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

impl std::fmt::Debug for ModelToolDefinition {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ModelToolDefinition")
            .field("name", &self.name)
            .field("description", &"[redacted tool description]")
            .field("input_schema", &"[redacted tool schema]")
            .finish()
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelChatRequest {
    pub messages: Vec<ModelMessage>,
    pub structured_output: Option<StructuredOutputRequest>,
    pub tools: Vec<ModelToolDefinition>,
}

impl std::fmt::Debug for ModelChatRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ModelChatRequest")
            .field("message_count", &self.messages.len())
            .field(
                "structured_output",
                &self.structured_output.as_ref().map(|value| &value.name),
            )
            .field(
                "tool_names",
                &self
                    .tools
                    .iter()
                    .map(|tool| tool.name.as_str())
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

impl std::fmt::Debug for ModelToolCall {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ModelToolCall")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("arguments", &"[redacted tool arguments]")
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelTokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelChatResponse {
    pub content: String,
    pub structured_output: Option<serde_json::Value>,
    pub tool_calls: Vec<ModelToolCall>,
    pub usage: ModelTokenUsage,
}

impl std::fmt::Debug for ModelChatResponse {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ModelChatResponse")
            .field("content", &"[redacted model output]")
            .field(
                "structured_output",
                &self.structured_output.as_ref().map(|_| "[redacted]"),
            )
            .field("tool_calls", &self.tool_calls)
            .field("usage", &self.usage)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelEmbeddingRequest {
    pub inputs: Vec<String>,
}

impl std::fmt::Debug for ModelEmbeddingRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ModelEmbeddingRequest")
            .field("input_count", &self.inputs.len())
            .field("inputs", &"[redacted embedding inputs]")
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelEmbeddingResponse {
    pub embeddings: Vec<Vec<f32>>,
    pub input_tokens: u64,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelStreamChunk {
    pub sequence: u64,
    pub content_delta: String,
    pub tool_calls: Vec<ModelToolCall>,
    pub done: bool,
}

impl std::fmt::Debug for ModelStreamChunk {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ModelStreamChunk")
            .field("sequence", &self.sequence)
            .field("content_delta", &"[redacted model output]")
            .field("tool_calls", &self.tool_calls)
            .field("done", &self.done)
            .finish()
    }
}
