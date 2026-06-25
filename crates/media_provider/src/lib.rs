use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ImageGenerationRequest {
    pub room_id: Uuid,
    pub prompt_version: String,
    pub safe_for_players: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ImageDraft {
    pub id: Uuid,
    pub status: ImageDraftStatus,
    pub object_key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ImageDraftStatus {
    Draft,
    Approved,
    Published,
    Rejected,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ImageProviderError {
    #[error("image generation is disabled")]
    Disabled,
}

#[async_trait]
pub trait ImageProvider: Send + Sync {
    async fn generate(
        &self,
        request: ImageGenerationRequest,
    ) -> Result<ImageDraft, ImageProviderError>;
}

#[derive(Debug, Clone, Default)]
pub struct MockImageProvider;

#[async_trait]
impl ImageProvider for MockImageProvider {
    async fn generate(
        &self,
        request: ImageGenerationRequest,
    ) -> Result<ImageDraft, ImageProviderError> {
        Ok(ImageDraft {
            id: request.room_id,
            status: ImageDraftStatus::Draft,
            object_key: "mock/draft.png".to_owned(),
        })
    }
}
