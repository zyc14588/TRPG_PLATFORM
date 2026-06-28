use async_trait::async_trait;
use auth::{RoomPrivacyMode, VisibilityScope};
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::Mutex;
use std::time::SystemTime;
use uuid::Uuid;

pub type Visibility = VisibilityScope;

pub const DEFAULT_MAX_TOP_K: u8 = 15;
pub const DEFAULT_MAX_CHUNK_CHARS: usize = 1_200;
pub const DEFAULT_MAX_RAW_TEXT_CHARS: usize = 200_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PrivacyMode {
    LocalOnly,
    AllowConfiguredCloud,
}

impl From<RoomPrivacyMode> for PrivacyMode {
    fn from(value: RoomPrivacyMode) -> Self {
        match value {
            RoomPrivacyMode::LocalOnly => Self::LocalOnly,
            RoomPrivacyMode::Standard | RoomPrivacyMode::PrivateHybrid => {
                Self::AllowConfiguredCloud
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    OfficialSrd,
    OpenText,
    UserProvidedText,
    CampaignNotes,
    CharacterSheet,
    ModulePrivateNotes,
    KpPrivateModule,
    CommercialAdapterMetadata,
    SystemInternal,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LicenseStatus {
    Allowed,
    PendingReview,
    Denied,
}

impl LicenseStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Allowed => "allowed",
            Self::PendingReview => "pending_review",
            Self::Denied => "denied",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LicenseDecision {
    pub status: LicenseStatus,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LicenseCheckInput {
    pub source_kind: SourceKind,
    pub license_name: Option<String>,
    pub declared_has_rights: bool,
    pub contains_commercial_rule_text: bool,
}

pub fn decide_license(input: &LicenseCheckInput) -> LicenseDecision {
    if input.contains_commercial_rule_text {
        return LicenseDecision {
            status: LicenseStatus::Denied,
            reason: "commercial rule prose is not allowed in adapters".to_owned(),
        };
    }
    if is_denied_license(input.license_name.as_deref()) {
        return LicenseDecision {
            status: LicenseStatus::Denied,
            reason: "license is incompatible with redistribution".to_owned(),
        };
    }

    match input.source_kind {
        SourceKind::OfficialSrd => LicenseDecision {
            status: LicenseStatus::Allowed,
            reason: "official SRD/open rules source".to_owned(),
        },
        SourceKind::CommercialAdapterMetadata => LicenseDecision {
            status: LicenseStatus::Allowed,
            reason: "adapter metadata contains mechanics/schema only".to_owned(),
        },
        SourceKind::UserProvidedText if input.declared_has_rights => LicenseDecision {
            status: LicenseStatus::Allowed,
            reason: "user declared rights".to_owned(),
        },
        SourceKind::CampaignNotes | SourceKind::CharacterSheet | SourceKind::KpPrivateModule
            if input.declared_has_rights =>
        {
            LicenseDecision {
                status: LicenseStatus::Allowed,
                reason: "user declared rights for room-scoped content".to_owned(),
            }
        }
        SourceKind::SystemInternal => LicenseDecision {
            status: LicenseStatus::Allowed,
            reason: "system internal generated metadata".to_owned(),
        },
        SourceKind::OpenText if is_recognized_open_license(input.license_name.as_deref()) => {
            LicenseDecision {
                status: LicenseStatus::Allowed,
                reason: "recognized open license".to_owned(),
            }
        }
        _ if input.declared_has_rights => LicenseDecision {
            status: LicenseStatus::Allowed,
            reason: "user declared rights".to_owned(),
        },
        _ if is_recognized_open_license(input.license_name.as_deref()) => LicenseDecision {
            status: LicenseStatus::Allowed,
            reason: "recognized open license".to_owned(),
        },
        _ => LicenseDecision {
            status: LicenseStatus::PendingReview,
            reason: "license missing or unclear".to_owned(),
        },
    }
}

pub fn check_declared_license(
    license_name: Option<&str>,
    declared_has_rights: bool,
) -> LicenseDecision {
    decide_license(&LicenseCheckInput {
        source_kind: SourceKind::Unknown,
        license_name: license_name.map(str::to_owned),
        declared_has_rights,
        contains_commercial_rule_text: false,
    })
}

fn is_recognized_open_license(license_name: Option<&str>) -> bool {
    let Some(name) = license_name else {
        return false;
    };
    let name = name.trim().to_ascii_lowercase();
    name.contains("cc0")
        || name.contains("cc-by")
        || name == "orc"
        || name.contains("open gaming license")
        || name == "ogl"
}

fn is_denied_license(license_name: Option<&str>) -> bool {
    let Some(name) = license_name else {
        return false;
    };
    let name = name.trim().to_ascii_lowercase();
    name.contains("no redistribution")
        || name.contains("no-redistribution")
        || name.contains("all rights reserved")
        || name.contains("non-commercial")
        || name.contains("noncommercial")
        || name.contains("cc-by-nc")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DocumentType {
    Rulebook,
    Module,
    Clue,
    SessionLog,
    Memory,
    CharacterSheet,
    CommercialAdapterMetadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IngestJobStatus {
    Queued,
    Claimed,
    Parsing,
    Embedding,
    Indexed,
    Completed,
    PendingReview,
    Denied,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DocumentSource {
    pub id: Uuid,
    pub room_id: Option<Uuid>,
    pub source_kind: SourceKind,
    pub title: String,
    pub license_status: LicenseStatus,
    pub license_reason: String,
    pub created_by: Option<Uuid>,
    pub visibility_default: VisibilityScope,
    pub metadata: Value,
    pub created_at: SystemTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Document {
    pub id: Uuid,
    pub source_id: Uuid,
    pub room_id: Option<Uuid>,
    pub title: String,
    pub normalized_hash: String,
    pub license_status: LicenseStatus,
    pub visibility: VisibilityScope,
    pub provider_metadata: ProviderMetadata,
    pub created_at: SystemTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Citation {
    pub source_title: String,
    pub section_path: Vec<String>,
    pub location_hint: Option<String>,
    pub content_hash: String,
    pub source_url: Option<String>,
    pub page_start: Option<i32>,
    pub page_end: Option<i32>,
    pub span: Option<String>,
    pub license_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ChunkDraft {
    pub document_id: Uuid,
    pub source_id: Uuid,
    pub room_id: Option<Uuid>,
    pub ordinal: u32,
    pub heading_path: Vec<String>,
    pub normalized_text: String,
    pub content_hash: String,
    pub license_status: LicenseStatus,
    pub visibility: VisibilityScope,
    pub token_estimate: u32,
    pub citation: Citation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Chunk {
    pub id: Uuid,
    pub document_id: Uuid,
    pub source_id: Uuid,
    pub room_id: Option<Uuid>,
    pub ordinal: u32,
    pub heading_path: Vec<String>,
    pub normalized_text: String,
    pub content_hash: String,
    pub license_status: LicenseStatus,
    pub visibility: VisibilityScope,
    pub token_estimate: u32,
    pub citation: Citation,
}

impl ChunkDraft {
    pub fn into_chunk(self, id: Uuid) -> Chunk {
        Chunk {
            id,
            document_id: self.document_id,
            source_id: self.source_id,
            room_id: self.room_id,
            ordinal: self.ordinal,
            heading_path: self.heading_path,
            normalized_text: self.normalized_text,
            content_hash: self.content_hash,
            license_status: self.license_status,
            visibility: self.visibility,
            token_estimate: self.token_estimate,
            citation: self.citation,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct IndexedChunk {
    pub chunk: Chunk,
    pub document_type: DocumentType,
    pub embedding: Vec<f32>,
    pub source_metadata: Value,
    pub provider_metadata: ProviderMetadata,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Evidence {
    pub source_id: Uuid,
    pub document_id: Uuid,
    pub chunk_id: Uuid,
    pub content_hash: String,
    pub score: f32,
    pub citation: Citation,
    pub preview_text: String,
    pub visibility: VisibilityScope,
    pub source_metadata: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct VisibilityMetadata {
    pub scope: VisibilityScope,
}

impl Evidence {
    pub fn from_scored_chunk(scored: ScoredChunk) -> Self {
        let indexed = scored.indexed_chunk;
        let chunk = indexed.chunk;
        Self {
            source_id: chunk.source_id,
            document_id: chunk.document_id,
            chunk_id: chunk.id,
            content_hash: chunk.content_hash.clone(),
            score: scored.score,
            citation: chunk.citation,
            preview_text: preview_text(&chunk.normalized_text),
            visibility: chunk.visibility,
            source_metadata: with_provider_metadata(
                indexed.source_metadata,
                &indexed.provider_metadata,
            ),
        }
    }

    pub fn safe_visibility_metadata(&self) -> VisibilityMetadata {
        VisibilityMetadata {
            scope: self.visibility,
        }
    }

    pub fn provider_metadata(&self) -> Option<ProviderMetadata> {
        self.source_metadata
            .get("provider_metadata")
            .cloned()
            .and_then(|value| serde_json::from_value(value).ok())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct TopK(u8);

impl TopK {
    pub fn new(value: u8, max: u8) -> Result<Self> {
        if value == 0 || value > max {
            return Err(RagError::TopKTooLarge {
                requested: value,
                max,
            });
        }
        Ok(Self(value))
    }

    pub fn get(self) -> u8 {
        self.0
    }
}

impl Default for TopK {
    fn default() -> Self {
        Self(DEFAULT_MAX_TOP_K)
    }
}

impl<'de> Deserialize<'de> for TopK {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u8::deserialize(deserializer)?;
        Self::new(value, DEFAULT_MAX_TOP_K).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RetrievalFilter {
    pub requester_id: Uuid,
    pub room_id: Option<Uuid>,
    pub session_id: Option<Uuid>,
    pub system_name: Option<String>,
    pub visibility_scopes: Vec<VisibilityScope>,
    pub document_types: Vec<DocumentType>,
    pub top_k: TopK,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RetrievalQuery {
    pub actor_id: Uuid,
    pub room_id: Option<Uuid>,
    pub query_text: String,
    pub top_k: TopK,
    pub filters: RetrievalFilter,
    pub privacy_mode: PrivacyMode,
    pub trace_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RetrievalResult {
    pub evidence: Vec<Evidence>,
    pub applied_filters: RetrievalFilter,
    pub provider_metadata: ProviderMetadata,
    pub trace_id: Option<Uuid>,
}

impl RetrievalResult {
    pub fn new(
        mut evidence: Vec<Evidence>,
        applied_filters: RetrievalFilter,
        provider_metadata: ProviderMetadata,
        trace_id: Option<Uuid>,
    ) -> Self {
        evidence.truncate(applied_filters.top_k.get() as usize);
        Self {
            evidence,
            applied_filters,
            provider_metadata,
            trace_id,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProviderLocation {
    Local,
    Cloud,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProviderMetadata {
    pub provider_kind: String,
    pub provider_version: Option<String>,
    pub model: Option<String>,
    pub location: ProviderLocation,
    pub dimension: Option<u16>,
    pub request_id: Option<String>,
}

impl ProviderMetadata {
    pub fn deterministic_local(dimension: u16) -> Self {
        Self {
            provider_kind: "deterministic_local".to_owned(),
            provider_version: Some("v1".to_owned()),
            model: Some("hash-bow-v1".to_owned()),
            location: ProviderLocation::Local,
            dimension: Some(dimension),
            request_id: None,
        }
    }

    pub fn cloud(
        provider_kind: impl Into<String>,
        model: impl Into<String>,
        dimension: u16,
    ) -> Self {
        Self {
            provider_kind: provider_kind.into(),
            provider_version: None,
            model: Some(model.into()),
            location: ProviderLocation::Cloud,
            dimension: Some(dimension),
            request_id: None,
        }
    }

    pub fn ensure_privacy_mode(self, privacy_mode: PrivacyMode) -> Result<Self> {
        if privacy_mode == PrivacyMode::LocalOnly && self.location == ProviderLocation::Cloud {
            return Err(RagError::ProviderRejectedPrivacyMode);
        }
        Ok(self)
    }
}

impl Default for ProviderMetadata {
    fn default() -> Self {
        Self::deterministic_local(0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ChunkingOptions {
    pub max_chunk_chars: usize,
    pub max_raw_text_chars: usize,
    pub max_chunks: usize,
}

impl Default for ChunkingOptions {
    fn default() -> Self {
        Self {
            max_chunk_chars: DEFAULT_MAX_CHUNK_CHARS,
            max_raw_text_chars: DEFAULT_MAX_RAW_TEXT_CHARS,
            max_chunks: 1_000,
        }
    }
}

impl ChunkingOptions {
    pub fn validate(&self) -> Result<()> {
        if self.max_chunk_chars == 0 {
            return Err(RagError::ChunkTooLarge {
                limit: 0,
                actual: 0,
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DocumentInput {
    pub document_id: Uuid,
    pub source_id: Uuid,
    pub room_id: Option<Uuid>,
    pub title: String,
    pub raw_text: String,
    pub license_status: LicenseStatus,
    pub visibility: VisibilityScope,
    pub source_url: Option<String>,
    pub license_name: Option<String>,
    pub metadata: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Embedding {
    pub chunk_ordinal: u32,
    pub vector: Vec<f32>,
    pub provider_metadata: ProviderMetadata,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EmbeddedQuery {
    pub vector: Vec<f32>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ProviderContext {
    pub privacy_mode: PrivacyMode,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ScoredChunk {
    pub indexed_chunk: IndexedChunk,
    pub score: f32,
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum RagError {
    #[error("license denied: {0}")]
    LicenseDenied(String),
    #[error("license pending review: {0}")]
    LicensePendingReview(String),
    #[error("visibility denied")]
    VisibilityDenied,
    #[error("provider rejected privacy mode")]
    ProviderRejectedPrivacyMode,
    #[error("provider unavailable: {0}")]
    ProviderUnavailable(String),
    #[error("chunk too large: {actual} > {limit}")]
    ChunkTooLarge { limit: usize, actual: usize },
    #[error("top_k too large: {requested} > {max}")]
    TopKTooLarge { requested: u8, max: u8 },
    #[error("invalid source metadata: {0}")]
    InvalidSourceMetadata(String),
    #[error("storage conflict: {0}")]
    StorageConflict(String),
    #[error("forbidden")]
    Forbidden,
}

pub type Result<T> = std::result::Result<T, RagError>;

#[async_trait]
pub trait Chunker: Send + Sync {
    async fn chunk(
        &self,
        input: &DocumentInput,
        options: ChunkingOptions,
    ) -> Result<Vec<ChunkDraft>>;
}

#[async_trait]
pub trait Embedder: Send + Sync {
    fn metadata(&self) -> ProviderMetadata;

    async fn embed(&self, input: &[ChunkDraft], ctx: ProviderContext) -> Result<Vec<Embedding>>;

    async fn embed_query(&self, query_text: &str, ctx: ProviderContext) -> Result<EmbeddedQuery>;
}

#[async_trait]
pub trait VectorStore: Send + Sync {
    async fn upsert(&self, chunks: &[IndexedChunk]) -> Result<()>;
    async fn search(
        &self,
        query: EmbeddedQuery,
        filter: RetrievalFilter,
    ) -> Result<Vec<ScoredChunk>>;
}

#[async_trait]
pub trait KeywordIndex: Send + Sync {
    async fn upsert(&self, chunks: &[IndexedChunk]) -> Result<()>;
    async fn search(&self, query: &str, filter: RetrievalFilter) -> Result<Vec<ScoredChunk>>;
}

#[async_trait]
pub trait HybridRetriever: Send + Sync {
    async fn retrieve(&self, query: RetrievalQuery) -> Result<RetrievalResult>;
}

#[derive(Debug, Clone, Default)]
pub struct MarkdownChunker;

#[async_trait]
impl Chunker for MarkdownChunker {
    async fn chunk(
        &self,
        input: &DocumentInput,
        options: ChunkingOptions,
    ) -> Result<Vec<ChunkDraft>> {
        options.validate()?;
        if input.raw_text.chars().count() > options.max_raw_text_chars {
            return Err(RagError::ChunkTooLarge {
                limit: options.max_raw_text_chars,
                actual: input.raw_text.chars().count(),
            });
        }
        match input.license_status {
            LicenseStatus::Allowed => {}
            LicenseStatus::PendingReview => {
                return Err(RagError::LicensePendingReview(
                    "source must be approved before chunking".to_owned(),
                ));
            }
            LicenseStatus::Denied => {
                return Err(RagError::LicenseDenied(
                    "denied source cannot be chunked".to_owned(),
                ));
            }
        }

        let normalized = normalize_text(&input.raw_text);
        let mut heading_path = Vec::<String>::new();
        let mut current = String::new();
        let mut drafts = Vec::new();

        for line in normalized.lines() {
            if let Some((level, heading)) = parse_markdown_heading(line) {
                flush_chunk(input, &heading_path, &mut current, &mut drafts, &options)?;
                while heading_path.len() >= level {
                    heading_path.pop();
                }
                heading_path.push(heading.to_owned());
                continue;
            }

            push_piece(
                input,
                &heading_path,
                line,
                &mut current,
                &mut drafts,
                &options,
            )?;
        }

        flush_chunk(input, &heading_path, &mut current, &mut drafts, &options)?;
        Ok(drafts)
    }
}

fn parse_markdown_heading(line: &str) -> Option<(usize, &str)> {
    let trimmed = line.trim_start();
    let level = trimmed.chars().take_while(|ch| *ch == '#').count();
    if !(1..=6).contains(&level) {
        return None;
    }
    let rest = trimmed[level..].trim_start();
    if rest.is_empty() {
        None
    } else {
        Some((level, rest))
    }
}

fn push_piece(
    input: &DocumentInput,
    heading_path: &[String],
    piece: &str,
    current: &mut String,
    drafts: &mut Vec<ChunkDraft>,
    options: &ChunkingOptions,
) -> Result<()> {
    let mut parts = split_by_char_limit(piece, options.max_chunk_chars);
    if parts.is_empty() {
        parts.push(String::new());
    }

    for part in parts {
        let separator = if current.is_empty() { "" } else { "\n" };
        let next_len = current.chars().count() + separator.chars().count() + part.chars().count();
        if next_len > options.max_chunk_chars && !current.is_empty() {
            flush_chunk(input, heading_path, current, drafts, options)?;
        }
        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(&part);
    }
    Ok(())
}

fn split_by_char_limit(value: &str, limit: usize) -> Vec<String> {
    if value.chars().count() <= limit {
        return vec![value.to_owned()];
    }

    let mut chunks = Vec::new();
    let mut current = String::new();
    for ch in value.chars() {
        if current.chars().count() >= limit {
            chunks.push(std::mem::take(&mut current));
        }
        current.push(ch);
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

fn flush_chunk(
    input: &DocumentInput,
    heading_path: &[String],
    current: &mut String,
    drafts: &mut Vec<ChunkDraft>,
    options: &ChunkingOptions,
) -> Result<()> {
    let normalized_text = normalize_text(current).trim().to_owned();
    current.clear();
    if normalized_text.is_empty() {
        return Ok(());
    }
    if normalized_text.chars().count() > options.max_chunk_chars {
        return Err(RagError::ChunkTooLarge {
            limit: options.max_chunk_chars,
            actual: normalized_text.chars().count(),
        });
    }
    if drafts.len() >= options.max_chunks {
        return Err(RagError::ChunkTooLarge {
            limit: options.max_chunks,
            actual: drafts.len() + 1,
        });
    }

    let content_hash = hash_normalized_text(&normalized_text);
    let token_estimate = estimate_tokens(&normalized_text);
    let ordinal = drafts.len() as u32;
    drafts.push(ChunkDraft {
        document_id: input.document_id,
        source_id: input.source_id,
        room_id: input.room_id,
        ordinal,
        heading_path: heading_path.to_vec(),
        normalized_text,
        content_hash: content_hash.clone(),
        license_status: input.license_status,
        visibility: input.visibility,
        token_estimate,
        citation: Citation {
            source_title: input.title.clone(),
            section_path: heading_path.to_vec(),
            location_hint: Some(format!("chunk {}", ordinal + 1)),
            content_hash,
            source_url: input.source_url.clone(),
            page_start: None,
            page_end: None,
            span: None,
            license_name: input.license_name.clone(),
        },
    });
    Ok(())
}

#[derive(Debug, Clone)]
pub struct DeterministicLocalEmbedder {
    dimension: u16,
}

impl Default for DeterministicLocalEmbedder {
    fn default() -> Self {
        Self { dimension: 16 }
    }
}

impl DeterministicLocalEmbedder {
    pub fn new(dimension: u16) -> Self {
        Self {
            dimension: dimension.max(1),
        }
    }

    pub fn embed_text(&self, text: &str) -> Vec<f32> {
        let normalized = normalize_text(text);
        let mut vector = vec![0.0; self.dimension as usize];
        for token in normalized.split_whitespace() {
            let digest = Sha256::digest(token.as_bytes());
            let index = (digest[0] as usize) % vector.len();
            vector[index] += 1.0;
        }
        normalize_vector(&mut vector);
        vector
    }
}

#[async_trait]
impl Embedder for DeterministicLocalEmbedder {
    fn metadata(&self) -> ProviderMetadata {
        ProviderMetadata::deterministic_local(self.dimension)
    }

    async fn embed(&self, input: &[ChunkDraft], ctx: ProviderContext) -> Result<Vec<Embedding>> {
        self.metadata().ensure_privacy_mode(ctx.privacy_mode)?;
        Ok(input
            .iter()
            .map(|chunk| Embedding {
                chunk_ordinal: chunk.ordinal,
                vector: self.embed_text(&chunk.normalized_text),
                provider_metadata: self.metadata(),
            })
            .collect())
    }

    async fn embed_query(&self, query_text: &str, ctx: ProviderContext) -> Result<EmbeddedQuery> {
        self.metadata().ensure_privacy_mode(ctx.privacy_mode)?;
        Ok(EmbeddedQuery {
            vector: self.embed_text(query_text),
            text: normalize_text(query_text),
        })
    }
}

#[derive(Debug, Default)]
pub struct InMemoryVectorStore {
    chunks: Mutex<Vec<IndexedChunk>>,
}

impl InMemoryVectorStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl VectorStore for InMemoryVectorStore {
    async fn upsert(&self, chunks: &[IndexedChunk]) -> Result<()> {
        for indexed in chunks {
            match indexed.chunk.license_status {
                LicenseStatus::Allowed => {}
                LicenseStatus::PendingReview => {
                    return Err(RagError::LicensePendingReview(
                        "pending chunks cannot be indexed".to_owned(),
                    ));
                }
                LicenseStatus::Denied => {
                    return Err(RagError::LicenseDenied(
                        "denied chunks cannot be indexed".to_owned(),
                    ));
                }
            }
        }

        let mut stored = self
            .chunks
            .lock()
            .map_err(|_| RagError::StorageConflict("vector store lock poisoned".to_owned()))?;
        for incoming in chunks {
            if let Some(existing) = stored
                .iter_mut()
                .find(|stored| stored.chunk.id == incoming.chunk.id)
            {
                *existing = incoming.clone();
            } else {
                stored.push(incoming.clone());
            }
        }
        Ok(())
    }

    async fn search(
        &self,
        query: EmbeddedQuery,
        filter: RetrievalFilter,
    ) -> Result<Vec<ScoredChunk>> {
        let stored = self
            .chunks
            .lock()
            .map_err(|_| RagError::StorageConflict("vector store lock poisoned".to_owned()))?;
        let mut scored = Vec::new();
        for indexed in stored.iter() {
            if !passes_filter(indexed, &filter) {
                continue;
            }
            scored.push(ScoredChunk {
                indexed_chunk: indexed.clone(),
                score: cosine_similarity(&query.vector, &indexed.embedding),
            });
        }
        scored.sort_by(|left, right| right.score.total_cmp(&left.score));
        scored.truncate(filter.top_k.get() as usize);
        Ok(scored)
    }
}

#[derive(Debug, Default)]
pub struct SimpleKeywordIndex {
    chunks: Mutex<Vec<IndexedChunk>>,
}

impl SimpleKeywordIndex {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl KeywordIndex for SimpleKeywordIndex {
    async fn upsert(&self, chunks: &[IndexedChunk]) -> Result<()> {
        for indexed in chunks {
            match indexed.chunk.license_status {
                LicenseStatus::Allowed => {}
                LicenseStatus::PendingReview => {
                    return Err(RagError::LicensePendingReview(
                        "pending chunks cannot be indexed".to_owned(),
                    ));
                }
                LicenseStatus::Denied => {
                    return Err(RagError::LicenseDenied(
                        "denied chunks cannot be indexed".to_owned(),
                    ));
                }
            }
        }

        let mut stored = self
            .chunks
            .lock()
            .map_err(|_| RagError::StorageConflict("keyword index lock poisoned".to_owned()))?;
        stored.extend_from_slice(chunks);
        Ok(())
    }

    async fn search(&self, query: &str, filter: RetrievalFilter) -> Result<Vec<ScoredChunk>> {
        let query_terms = lower_terms(query);
        let stored = self
            .chunks
            .lock()
            .map_err(|_| RagError::StorageConflict("keyword index lock poisoned".to_owned()))?;
        let mut scored = Vec::new();
        for indexed in stored.iter() {
            if !passes_filter(indexed, &filter) {
                continue;
            }
            let text = indexed.chunk.normalized_text.to_ascii_lowercase();
            let matches = query_terms
                .iter()
                .filter(|term| text.contains(term.as_str()))
                .count();
            if matches > 0 {
                scored.push(ScoredChunk {
                    indexed_chunk: indexed.clone(),
                    score: matches as f32,
                });
            }
        }
        scored.sort_by(|left, right| right.score.total_cmp(&left.score));
        scored.truncate(filter.top_k.get() as usize);
        Ok(scored)
    }
}

#[derive(Debug)]
pub struct LocalHybridRetriever<E, V, K> {
    embedder: E,
    vector_store: V,
    keyword_index: K,
}

impl<E, V, K> LocalHybridRetriever<E, V, K> {
    pub fn new(embedder: E, vector_store: V, keyword_index: K) -> Self {
        Self {
            embedder,
            vector_store,
            keyword_index,
        }
    }
}

#[async_trait]
impl<E, V, K> HybridRetriever for LocalHybridRetriever<E, V, K>
where
    E: Embedder,
    V: VectorStore,
    K: KeywordIndex,
{
    async fn retrieve(&self, query: RetrievalQuery) -> Result<RetrievalResult> {
        let embedded_query = self
            .embedder
            .embed_query(
                &query.query_text,
                ProviderContext {
                    privacy_mode: query.privacy_mode,
                },
            )
            .await?;
        let filter = query.filters.clone();
        let mut merged = self
            .vector_store
            .search(embedded_query, filter.clone())
            .await?;
        let keyword_hits = self
            .keyword_index
            .search(&query.query_text, filter.clone())
            .await?;

        // ponytail: O(n^2) merge is fine for the local test provider; use a map if this grows.
        for hit in keyword_hits {
            if let Some(existing) = merged
                .iter_mut()
                .find(|existing| existing.indexed_chunk.chunk.id == hit.indexed_chunk.chunk.id)
            {
                existing.score = existing.score.max(hit.score);
            } else {
                merged.push(hit);
            }
        }

        merged.sort_by(|left, right| {
            right.score.total_cmp(&left.score).then_with(|| {
                left.indexed_chunk
                    .chunk
                    .ordinal
                    .cmp(&right.indexed_chunk.chunk.ordinal)
            })
        });
        merged.truncate(filter.top_k.get() as usize);
        let evidence = merged
            .into_iter()
            .map(Evidence::from_scored_chunk)
            .collect();

        Ok(RetrievalResult::new(
            evidence,
            query.filters,
            self.embedder.metadata(),
            query.trace_id,
        ))
    }
}

fn passes_filter(indexed: &IndexedChunk, filter: &RetrievalFilter) -> bool {
    let chunk = &indexed.chunk;
    if chunk.license_status != LicenseStatus::Allowed {
        return false;
    }
    if chunk.visibility == VisibilityScope::SystemInternal {
        return false;
    }
    if !filter.visibility_scopes.contains(&chunk.visibility) {
        return false;
    }
    if !filter.document_types.is_empty() && !filter.document_types.contains(&indexed.document_type)
    {
        return false;
    }
    match (filter.room_id, chunk.room_id) {
        (Some(expected), Some(actual)) if expected != actual => false,
        (None, Some(_)) => false,
        _ => true,
    }
}

pub fn normalize_text(raw: &str) -> String {
    raw.replace("\r\n", "\n")
        .replace('\r', "\n")
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn hash_normalized_text(normalized: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

fn preview_text(text: &str) -> String {
    const PREVIEW_CHARS: usize = 240;
    text.chars().take(PREVIEW_CHARS).collect()
}

fn with_provider_metadata(source_metadata: Value, provider_metadata: &ProviderMetadata) -> Value {
    let mut map = match source_metadata {
        Value::Object(map) => map,
        Value::Null => serde_json::Map::new(),
        other => {
            let mut map = serde_json::Map::new();
            map.insert("source".to_owned(), other);
            map
        }
    };
    if let Ok(value) = serde_json::to_value(provider_metadata) {
        map.insert("provider_metadata".to_owned(), value);
    }
    Value::Object(map)
}

fn estimate_tokens(text: &str) -> u32 {
    text.chars().count().div_ceil(4).max(1) as u32
}

fn normalize_vector(vector: &mut [f32]) {
    let magnitude = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if magnitude > 0.0 {
        for value in vector {
            *value /= magnitude;
        }
    }
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    left.iter()
        .zip(right)
        .map(|(left, right)| left * right)
        .sum()
}

fn lower_terms(query: &str) -> Vec<String> {
    query
        .split_whitespace()
        .map(|term| term.to_ascii_lowercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(raw_text: &str) -> DocumentInput {
        DocumentInput {
            document_id: Uuid::from_u128(1),
            source_id: Uuid::from_u128(2),
            room_id: Some(Uuid::from_u128(3)),
            title: "Test Source".to_owned(),
            raw_text: raw_text.to_owned(),
            license_status: LicenseStatus::Allowed,
            visibility: VisibilityScope::RoomRule,
            source_url: Some("https://example.test/source".to_owned()),
            license_name: Some("CC-BY-4.0".to_owned()),
            metadata: Value::Null,
        }
    }

    fn filter(top_k: u8, scopes: Vec<VisibilityScope>) -> RetrievalFilter {
        RetrievalFilter {
            requester_id: Uuid::from_u128(9),
            room_id: Some(Uuid::from_u128(3)),
            session_id: None,
            system_name: None,
            visibility_scopes: scopes,
            document_types: vec![DocumentType::Rulebook],
            top_k: TopK::new(top_k, DEFAULT_MAX_TOP_K).expect("valid top_k"),
        }
    }

    #[test]
    fn license_allowed_pending_denied_semantics() {
        assert_eq!(
            decide_license(&LicenseCheckInput {
                source_kind: SourceKind::OfficialSrd,
                license_name: None,
                declared_has_rights: false,
                contains_commercial_rule_text: false,
            })
            .status,
            LicenseStatus::Allowed
        );
        assert_eq!(
            check_declared_license(None, false).status,
            LicenseStatus::PendingReview
        );
        assert_eq!(
            decide_license(&LicenseCheckInput {
                source_kind: SourceKind::CommercialAdapterMetadata,
                license_name: None,
                declared_has_rights: false,
                contains_commercial_rule_text: true,
            })
            .status,
            LicenseStatus::Denied
        );
    }

    #[test]
    fn privacy_mode_supports_local_only_and_allow_configured_cloud() {
        assert_eq!(
            PrivacyMode::from(RoomPrivacyMode::LocalOnly),
            PrivacyMode::LocalOnly
        );
        assert_eq!(
            PrivacyMode::from(RoomPrivacyMode::PrivateHybrid),
            PrivacyMode::AllowConfiguredCloud
        );
    }

    #[test]
    fn visibility_scope_supports_pl_kp_and_system_internal_boundaries() {
        let actor = auth::UserId(Uuid::from_u128(1));
        assert!(VisibilityScope::PublicRule.visible_to(auth::RoomRole::Pl, actor, None));
        assert!(!VisibilityScope::KpOnlyModule.visible_to(auth::RoomRole::Pl, actor, None));
        assert!(VisibilityScope::KpOnlyModule.visible_to(auth::RoomRole::Kp, actor, None));
        assert!(!VisibilityScope::SystemInternal.visible_to(auth::RoomRole::Kp, actor, None));
    }

    #[test]
    fn chunk_hash_stable() {
        let normalized = normalize_text("alpha\r\nbeta  ");
        assert_eq!(
            hash_normalized_text(&normalized),
            hash_normalized_text("alpha\nbeta")
        );
    }

    #[test]
    fn chunk_hash_changes_on_content_change() {
        assert_ne!(
            hash_normalized_text("same text"),
            hash_normalized_text("changed text")
        );
    }

    #[tokio::test]
    async fn markdown_heading_path_preserved() {
        let chunker = MarkdownChunker;
        let chunks = chunker
            .chunk(
                &input("# Combat\nRules.\n## Initiative\nRoll quickly."),
                ChunkingOptions::default(),
            )
            .await
            .expect("allowed text chunks");

        assert!(chunks
            .iter()
            .any(|chunk| chunk.heading_path == vec!["Combat", "Initiative"]));
    }

    #[tokio::test]
    async fn chunk_size_is_bounded() {
        let chunks = MarkdownChunker
            .chunk(
                &input("abcdefghij"),
                ChunkingOptions {
                    max_chunk_chars: 4,
                    ..ChunkingOptions::default()
                },
            )
            .await
            .expect("bounded chunks");

        assert!(chunks
            .iter()
            .all(|chunk| chunk.normalized_text.chars().count() <= 4));
    }

    #[tokio::test]
    async fn local_embedder_is_deterministic() {
        let embedder = DeterministicLocalEmbedder::default();
        let first = embedder
            .embed_query(
                "same query",
                ProviderContext {
                    privacy_mode: PrivacyMode::LocalOnly,
                },
            )
            .await
            .expect("first local query embed");
        let second = embedder
            .embed_query(
                "same query",
                ProviderContext {
                    privacy_mode: PrivacyMode::LocalOnly,
                },
            )
            .await
            .expect("second local query embed");

        assert_eq!(first, second);
    }

    #[test]
    fn local_only_rejects_cloud_provider() {
        let metadata = ProviderMetadata::cloud("cloud_embedder", "remote-model", 3);
        assert_eq!(
            metadata
                .ensure_privacy_mode(PrivacyMode::LocalOnly)
                .expect_err("cloud rejected"),
            RagError::ProviderRejectedPrivacyMode
        );
    }

    #[tokio::test]
    async fn citation_required_for_evidence() {
        let embedder = DeterministicLocalEmbedder::default();
        let chunk = MarkdownChunker
            .chunk(&input("# Rules\nvisible clue"), ChunkingOptions::default())
            .await
            .expect("chunk")
            .remove(0)
            .into_chunk(Uuid::from_u128(42));
        let indexed = IndexedChunk {
            embedding: embedder.embed_text(&chunk.normalized_text),
            chunk,
            document_type: DocumentType::Rulebook,
            source_metadata: serde_json::json!({"source_kind": "test"}),
            provider_metadata: embedder.metadata(),
        };
        let evidence = Evidence::from_scored_chunk(ScoredChunk {
            indexed_chunk: indexed,
            score: 1.0,
        });

        assert_eq!(evidence.source_id, Uuid::from_u128(2));
        assert_eq!(evidence.document_id, Uuid::from_u128(1));
        assert_eq!(evidence.chunk_id, Uuid::from_u128(42));
        assert!(!evidence.content_hash.is_empty());
        assert_eq!(evidence.citation.source_title, "Test Source");
        assert_eq!(
            evidence.safe_visibility_metadata().scope,
            VisibilityScope::RoomRule
        );
        assert_eq!(
            evidence
                .provider_metadata()
                .expect("provider metadata")
                .location,
            ProviderLocation::Local
        );
    }

    #[tokio::test]
    async fn local_hybrid_retriever_returns_evidence_without_network() {
        let embedder = DeterministicLocalEmbedder::default();
        let chunk = MarkdownChunker
            .chunk(
                &input("# Rules\ninitiative order"),
                ChunkingOptions::default(),
            )
            .await
            .expect("chunk")
            .remove(0)
            .into_chunk(Uuid::from_u128(43));
        let indexed = IndexedChunk {
            embedding: embedder.embed_text(&chunk.normalized_text),
            chunk,
            document_type: DocumentType::Rulebook,
            source_metadata: Value::Null,
            provider_metadata: embedder.metadata(),
        };
        let vector_store = InMemoryVectorStore::new();
        vector_store
            .upsert(std::slice::from_ref(&indexed))
            .await
            .expect("vector upsert");
        let keyword_index = SimpleKeywordIndex::new();
        keyword_index
            .upsert(&[indexed])
            .await
            .expect("keyword upsert");

        let retriever = LocalHybridRetriever::new(embedder, vector_store, keyword_index);
        let result = retriever
            .retrieve(RetrievalQuery {
                actor_id: Uuid::from_u128(9),
                room_id: Some(Uuid::from_u128(3)),
                query_text: "initiative".to_owned(),
                top_k: TopK::new(1, DEFAULT_MAX_TOP_K).expect("valid top_k"),
                filters: filter(1, vec![VisibilityScope::RoomRule]),
                privacy_mode: PrivacyMode::LocalOnly,
                trace_id: Some(Uuid::from_u128(99)),
            })
            .await
            .expect("retrieve");

        assert_eq!(result.evidence.len(), 1);
        assert_eq!(result.trace_id, Some(Uuid::from_u128(99)));
        assert_eq!(result.provider_metadata.location, ProviderLocation::Local);
    }

    #[test]
    fn license_decisions_cover_pending_denied_and_allowed() {
        assert_eq!(
            check_declared_license(None, false).status,
            LicenseStatus::PendingReview
        );
        assert_eq!(
            decide_license(&LicenseCheckInput {
                source_kind: SourceKind::CommercialAdapterMetadata,
                license_name: None,
                declared_has_rights: false,
                contains_commercial_rule_text: true,
            })
            .status,
            LicenseStatus::Denied
        );
        assert_eq!(
            decide_license(&LicenseCheckInput {
                source_kind: SourceKind::CommercialAdapterMetadata,
                license_name: None,
                declared_has_rights: false,
                contains_commercial_rule_text: false,
            })
            .status,
            LicenseStatus::Allowed
        );
        assert_eq!(
            check_declared_license(Some("CC-BY-NC-4.0"), true).status,
            LicenseStatus::Denied
        );
    }

    #[test]
    fn normalization_makes_crlf_and_trailing_space_hash_stable() {
        let crlf = normalize_text("alpha  \r\nbeta\t\r\n");
        let lf = normalize_text("alpha\nbeta");
        assert_eq!(crlf, lf);
        assert_eq!(hash_normalized_text(&crlf), hash_normalized_text(&lf));
    }

    #[tokio::test]
    async fn markdown_heading_path_preserved_and_chunk_size_bounded() {
        let chunker = MarkdownChunker;
        let chunks = chunker
            .chunk(
                &input("# Combat\nFirst paragraph.\n## Initiative\nRoll quickly."),
                ChunkingOptions {
                    max_chunk_chars: 18,
                    ..ChunkingOptions::default()
                },
            )
            .await
            .expect("allowed text chunks");

        assert!(chunks
            .iter()
            .all(|chunk| chunk.normalized_text.chars().count() <= 18));
        assert!(chunks
            .iter()
            .any(|chunk| chunk.citation.section_path == vec!["Combat", "Initiative"]));
    }

    #[tokio::test]
    async fn chunk_hashes_are_stable_and_change_with_content() {
        let chunker = MarkdownChunker;
        let first = chunker
            .chunk(&input("# Rules\nSame text."), ChunkingOptions::default())
            .await
            .expect("first chunks");
        let second = chunker
            .chunk(
                &input("# Rules\r\nSame text.  "),
                ChunkingOptions::default(),
            )
            .await
            .expect("second chunks");
        let changed = chunker
            .chunk(
                &input("# Rules\nDifferent text."),
                ChunkingOptions::default(),
            )
            .await
            .expect("changed chunks");

        assert_eq!(first[0].content_hash, second[0].content_hash);
        assert_ne!(first[0].content_hash, changed[0].content_hash);
    }

    #[tokio::test]
    async fn pending_and_denied_sources_are_not_chunked() {
        let chunker = MarkdownChunker;
        let mut pending = input("text");
        pending.license_status = LicenseStatus::PendingReview;
        let mut denied = input("text");
        denied.license_status = LicenseStatus::Denied;

        assert!(matches!(
            chunker
                .chunk(&pending, ChunkingOptions::default())
                .await
                .expect_err("pending rejected"),
            RagError::LicensePendingReview(_)
        ));
        assert!(matches!(
            chunker
                .chunk(&denied, ChunkingOptions::default())
                .await
                .expect_err("denied rejected"),
            RagError::LicenseDenied(_)
        ));
    }

    #[tokio::test]
    async fn deterministic_local_embedder_is_stable_and_local_only_safe() {
        let chunker = MarkdownChunker;
        let chunks = chunker
            .chunk(&input("same text"), ChunkingOptions::default())
            .await
            .expect("chunks");
        let embedder = DeterministicLocalEmbedder::default();
        let first = embedder
            .embed(
                &chunks,
                ProviderContext {
                    privacy_mode: PrivacyMode::LocalOnly,
                },
            )
            .await
            .expect("local embed");
        let second = embedder
            .embed(
                &chunks,
                ProviderContext {
                    privacy_mode: PrivacyMode::LocalOnly,
                },
            )
            .await
            .expect("local embed");

        assert_eq!(first, second);
    }

    #[test]
    fn local_only_rejects_cloud_provider_metadata() {
        let metadata = ProviderMetadata {
            provider_kind: "cloud".to_owned(),
            provider_version: None,
            model: Some("embedding-cloud".to_owned()),
            location: ProviderLocation::Cloud,
            dimension: Some(3),
            request_id: None,
        };

        assert_eq!(
            metadata
                .ensure_privacy_mode(PrivacyMode::LocalOnly)
                .expect_err("cloud rejected"),
            RagError::ProviderRejectedPrivacyMode
        );
    }

    #[test]
    fn top_k_is_bounded() {
        assert!(TopK::new(DEFAULT_MAX_TOP_K, DEFAULT_MAX_TOP_K).is_ok());
        assert!(matches!(
            TopK::new(DEFAULT_MAX_TOP_K + 1, DEFAULT_MAX_TOP_K),
            Err(RagError::TopKTooLarge { .. })
        ));
        assert!(serde_json::from_str::<TopK>(&format!("{}", DEFAULT_MAX_TOP_K + 1)).is_err());
    }

    #[tokio::test]
    async fn vector_store_filters_license_and_visibility_before_scoring() {
        let embedder = DeterministicLocalEmbedder::default();
        let query = embedder.embed_text("secret");
        let allowed_chunk = ChunkDraft {
            visibility: VisibilityScope::RoomRule,
            normalized_text: "boring public text".to_owned(),
            content_hash: hash_normalized_text("boring public text"),
            ..MarkdownChunker
                .chunk(&input("boring public text"), ChunkingOptions::default())
                .await
                .expect("public chunk")
                .remove(0)
        }
        .into_chunk(Uuid::from_u128(10));
        let denied_chunk = Chunk {
            id: Uuid::from_u128(11),
            license_status: LicenseStatus::Denied,
            normalized_text: "secret".to_owned(),
            content_hash: hash_normalized_text("secret"),
            ..allowed_chunk.clone()
        };
        let kp_chunk = Chunk {
            id: Uuid::from_u128(12),
            visibility: VisibilityScope::KpOnlyModule,
            normalized_text: "secret".to_owned(),
            content_hash: hash_normalized_text("secret"),
            ..allowed_chunk.clone()
        };
        let provider_metadata = embedder.metadata();
        let indexed = |chunk: Chunk| IndexedChunk {
            embedding: embedder.embed_text(&chunk.normalized_text),
            chunk,
            document_type: DocumentType::Rulebook,
            source_metadata: Value::Null,
            provider_metadata: provider_metadata.clone(),
        };
        let store = InMemoryVectorStore::new();
        store
            .upsert(&[indexed(allowed_chunk), indexed(kp_chunk)])
            .await
            .expect("allowed and kp chunks index");
        assert!(matches!(
            store.upsert(&[indexed(denied_chunk)]).await,
            Err(RagError::LicenseDenied(_))
        ));

        let results = store
            .search(
                EmbeddedQuery {
                    vector: query,
                    text: "secret".to_owned(),
                },
                filter(5, vec![VisibilityScope::RoomRule]),
            )
            .await
            .expect("search");

        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].indexed_chunk.chunk.visibility,
            VisibilityScope::RoomRule
        );
    }
}
