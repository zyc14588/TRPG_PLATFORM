# B01 Spec — RAG Core Domain Model

## Goal

Create a shared, provider-agnostic, no-network domain layer for P2. This layer should compile and test without PostgreSQL, HTTP server, frontend or live provider credentials.

## Required domain enums

Names may follow project style, but semantics must exist.

### `SourceKind`

- `OfficialSrd` / open licensed rules text
- `UserProvidedText`
- `CampaignNotes`
- `CharacterSheet`
- `KpPrivateModule`
- `CommercialAdapterMetadata`
- `SystemInternal`

### `LicenseStatus`

- `Allowed`
- `PendingReview`
- `Denied`

Semantics:

- `Allowed` may proceed to chunk/embed/index if visibility permits.
- `PendingReview` may be listed only through review path; not ordinary retrieval.
- `Denied` is terminal for ordinary indexing and retrieval.

### `VisibilityScope`

- `PublicRule`
- `RoomRule`
- `PlayerVisibleClue`
- `CharacterPrivate`
- `KpOnlyModule`
- `KpSecret`
- `MemoryPrivate`
- `SystemInternal`

Semantics:

- Visibility is an access-control predicate, not a frontend display flag.
- Visibility must be evaluated before scoring/ranking/prompt construction.

### `PrivacyMode`

- `LocalOnly`
- `AllowConfiguredCloud`

Semantics:

- LocalOnly rejects cloud providers for completion, embedding, reranking, OCR and image tasks.
- Provider selection must not be controlled by client-supplied secrets.

### `IngestJobStatus`

At minimum:

- `Queued` or `Claimed`
- `Parsing`
- `Embedding`
- `Indexed` or `Completed`
- `PendingReview`
- `Denied`
- `Failed`

Rust enum serialization and DB CHECK constraints must stay aligned. Missing `denied` in DB constraints is a P2 blocker.

## Required structs

### `DocumentSource`

Fields:

- `id`
- `room_id`
- `source_kind`
- `title`
- `license_status`
- `license_reason`
- `created_by`
- `visibility_default`
- `metadata`
- `created_at`

### `Document`

Fields:

- `id`
- `source_id`
- `room_id`
- `title`
- `normalized_hash`
- `license_status`
- `visibility`
- `provider_metadata`
- `created_at`

### `ChunkDraft` / `IndexedChunk`

Fields:

- `id` or deterministic draft identity
- `document_id`
- `source_id`
- `room_id`
- `ordinal`
- `heading_path`
- `normalized_text` or `content`
- `content_hash`
- `visibility`
- `token_estimate`
- `citation`

### `Citation`

Fields:

- source title
- section path / heading path
- location hint
- content hash
- optional URL
- optional page/span if future PDF support exists

### `Evidence`

Fields:

- source/document/chunk ids
- score
- preview text
- citation
- content hash
- safe source metadata
- safe visibility metadata
- provider metadata

### `RetrievalQuery`

Fields:

- actor id / room id / actor role or context reference
- query text
- bounded `top_k`
- filters
- privacy mode
- trace id if tracing exists

### `RetrievalResult`

Fields:

- evidence list
- applied filters
- provider metadata
- trace id

## Required traits

### `Chunker`

- deterministic
- size bounded
- heading-aware for Markdown/plain text
- no DB/network/env access

### `Embedder`

- async if project style requires
- returns deterministic local vectors for tests
- does not read env in `rag_core`
- honors `PrivacyMode`

### `VectorIndex` / `VectorStore`

- accepts already-authorized candidates only or requires explicit prefilter arguments
- no hidden/denied content in scoring set

### `Retriever`

- returns `RetrievalResult`
- requires prefiltered candidates or delegates to storage repository that applies prefilter
- never generates final answer

## Normalization and hashing

Required behavior:

- Normalize CRLF/CR to LF.
- Preserve heading structure.
- Use stable UTF-8 normalization rules chosen by project.
- Hash normalized content with SHA-256 or project standard.
- Same normalized content yields same hash.
- Content changes yield different hash.
- Hash should not depend on time, random numbers, absolute path or map iteration order.

## Error model

Required error variants or equivalents:

- `LicenseDenied`
- `LicensePendingReview`
- `VisibilityDenied`
- `ProviderRejectedPrivacyMode`
- `ProviderUnavailable`
- `ChunkTooLarge`
- `TopKTooLarge`
- `InvalidSourceMetadata`
- `StorageConflict`
- `Forbidden`

Security rejection must not be collapsed into opaque internal errors.

## Required tests

- `chunk_hash_stable`
- `chunk_hash_changes_on_content_change`
- `markdown_heading_path_preserved`
- `chunk_size_is_bounded`
- `license_allowed_pending_denied_semantics`
- `local_embedder_is_deterministic`
- `local_only_rejects_cloud_provider`
- `top_k_is_bounded`
- `citation_required_for_evidence`

## Batch boundary

B01 must not add migrations, public HTTP routes, frontend UI, live provider calls, or Rig agent workflows. Rig integration starts in B04.
