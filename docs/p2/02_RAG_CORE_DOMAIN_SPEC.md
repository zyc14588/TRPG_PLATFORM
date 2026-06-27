# RAG Core Domain Spec

## Crate boundary

`crates/rag_core` owns all shared RAG domain semantics. No other crate should define a second `LicenseStatus`, `Visibility`, `Chunk`, `Citation`, or `RetrievalResult` with incompatible meanings.

## Core enums

```rust
pub enum SourceKind {
    OfficialSrd,
    UserProvidedText,
    CampaignNotes,
    CharacterSheet,
    ModulePrivateNotes,
    CommercialAdapterMetadata,
}

pub enum LicenseStatus {
    Allowed,
    PendingReview,
    Denied,
}

pub enum Visibility {
    PublicRule,
    RoomRule,
    PlVisibleClue,
    CharacterPrivate,
    KpOnlyModule,
    KpSecret,
    MemoryPrivate,
    SystemInternal,
}

pub enum PrivacyMode {
    LocalOnly,
    AllowConfiguredCloud,
}
```

Names may vary to match existing code, but semantics must remain stable and documented.

## Core structs

Required fields:

- `DocumentSource`: id, room_id, source_kind, title, license_status, license_reason, created_by, visibility_default, metadata, created_at.
- `Document`: id, source_id, room_id, title, normalized_hash, license_status, visibility, provider_metadata, created_at.
- `Chunk`: id, document_id, source_id, room_id, ordinal, heading_path, normalized_text, content_hash, visibility, token_estimate, citation.
- `Citation`: source title, section path, location hint, content hash, optional URL, optional page/span.
- `Evidence`: chunk id, score, citation, preview text, visibility, source metadata.
- `RetrievalQuery`: actor, room, query text, top_k, filters, privacy mode.
- `RetrievalResult`: bounded list of evidence, applied filters, provider metadata, trace id.

## Traits

```rust
#[async_trait]
pub trait Chunker {
    async fn chunk(&self, doc: &DocumentInput, options: ChunkingOptions) -> Result<Vec<ChunkDraft>>;
}

#[async_trait]
pub trait Embedder {
    async fn embed(&self, input: &[ChunkDraft], ctx: ProviderContext) -> Result<Vec<Embedding>>;
}

#[async_trait]
pub trait VectorStore {
    async fn upsert(&self, chunks: &[IndexedChunk]) -> Result<()>;
    async fn search(&self, query: EmbeddedQuery, filter: RetrievalFilter) -> Result<Vec<ScoredChunk>>;
}

#[async_trait]
pub trait HybridRetriever {
    async fn retrieve(&self, query: RetrievalQuery) -> Result<RetrievalResult>;
}
```

## Deterministic local implementations

Required for tests:

- `MarkdownChunker`: preserves heading path; enforces max chunk size.
- `DeterministicLocalEmbedder`: no network; stable vector for same normalized text.
- `InMemoryVectorStore`: supports local smoke tests.
- Optional `SimpleKeywordIndex`: deterministic keyword scoring for hybrid ranking tests.

## Hashing and normalization

Normalize before hashing:

- Convert CRLF/CR to LF.
- Trim trailing whitespace per line.
- Collapse excessive blank lines only if documented.
- Preserve heading structure.
- Hash normalized UTF-8 with SHA-256 or existing project hash convention.

Required tests:

- same normalized text -> same hash
- changed content -> changed hash
- CRLF vs LF -> same hash if normalization says so
- heading path appears in citation

## Error model

Use explicit error variants:

- `LicenseDenied`
- `LicensePendingReview`
- `VisibilityDenied`
- `ProviderRejectedPrivacyMode`
- `ChunkTooLarge`
- `TopKTooLarge`
- `InvalidSourceMetadata`
- `StorageConflict`

Do not map security denials to internal server errors.
