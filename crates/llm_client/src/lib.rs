use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ModelProviderKind {
    OpenAiCompatible,
    Ollama,
    LlamaCpp,
    Mock,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ModelRef {
    pub provider: ModelProviderKind,
    pub model: String,
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
pub struct LlmUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub cached_tokens: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ChatJsonRequest {
    pub model: ModelRef,
    pub system: String,
    pub prompt: String,
    pub schema: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ChatJsonResponse {
    pub content: Value,
    pub usage: LlmUsage,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EmbeddingRequest {
    pub model: ModelRef,
    pub texts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EmbeddingResponse {
    pub vectors: Vec<Vec<f32>>,
    pub usage: LlmUsage,
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum LlmError {
    #[error("provider is disabled by privacy mode")]
    DisabledByPrivacyMode,
    #[error("schema validation failed: {0}")]
    Schema(String),
    #[error("provider transport failed: {0}")]
    Transport(String),
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn chat_json(&self, request: ChatJsonRequest) -> Result<ChatJsonResponse, LlmError>;
}

#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    async fn embed(&self, request: EmbeddingRequest) -> Result<EmbeddingResponse, LlmError>;
}

#[derive(Debug, Clone, Default)]
pub struct MockLlmProvider;

#[async_trait]
impl LlmProvider for MockLlmProvider {
    async fn chat_json(&self, request: ChatJsonRequest) -> Result<ChatJsonResponse, LlmError> {
        Ok(ChatJsonResponse {
            content: json!({
                "mock": true,
                "prompt": request.prompt,
            }),
            usage: LlmUsage::default(),
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct MockEmbeddingProvider;

#[async_trait]
impl EmbeddingProvider for MockEmbeddingProvider {
    async fn embed(&self, request: EmbeddingRequest) -> Result<EmbeddingResponse, LlmError> {
        let vectors = request
            .texts
            .iter()
            .map(|text| vec![text.len() as f32, 0.0, 1.0])
            .collect();

        Ok(EmbeddingResponse {
            vectors,
            usage: LlmUsage::default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_embedding_is_deterministic() {
        let provider = MockEmbeddingProvider;
        let model = ModelRef {
            provider: ModelProviderKind::Mock,
            model: "mock-embedding".to_owned(),
            base_url: None,
        };

        let response = provider
            .embed(EmbeddingRequest {
                model,
                texts: vec!["abc".to_owned()],
            })
            .await
            .expect("mock provider should not fail");

        assert_eq!(response.vectors, vec![vec![3.0, 0.0, 1.0]]);
    }
}
