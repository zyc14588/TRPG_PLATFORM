use async_trait::async_trait;
use auth::VisibilityScope;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DocumentType {
    Rulebook,
    Module,
    Clue,
    SessionLog,
    Memory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RetrievalFilter {
    pub requester_id: Uuid,
    pub room_id: Option<Uuid>,
    pub session_id: Option<Uuid>,
    pub system_name: Option<String>,
    pub visibility_scopes: Vec<VisibilityScope>,
    pub document_types: Vec<DocumentType>,
    pub top_k: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Evidence {
    pub chunk_id: Uuid,
    pub document_id: Uuid,
    pub title: String,
    pub section_path: Vec<String>,
    pub score: f32,
    pub preview: String,
    pub visibility_scope: VisibilityScope,
    pub citation: Citation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Citation {
    pub source_url: Option<String>,
    pub page_start: Option<i32>,
    pub page_end: Option<i32>,
    pub license_name: Option<String>,
}

#[async_trait]
pub trait VectorStore: Send + Sync {
    async fn upsert(&self, items: Vec<(Uuid, Vec<f32>)>) -> anyhow::Result<()>;
    async fn search(
        &self,
        query: &[f32],
        filter: &RetrievalFilter,
    ) -> anyhow::Result<Vec<(Uuid, f32)>>;
}

#[async_trait]
pub trait KeywordIndex: Send + Sync {
    async fn search(
        &self,
        query: &str,
        filter: &RetrievalFilter,
    ) -> anyhow::Result<Vec<(Uuid, f32)>>;
}
