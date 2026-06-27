# Storage, Migration, and RLS Spec

## Migration policy

- Additive migrations only.
- Do not rewrite existing shipped migrations.
- Every new table with room/document/chunk data must enable and force RLS unless explicitly documented otherwise.
- Every ordinary retrieval path must be DB-deny-by-default for `pending_review` and `denied` licenses.

## Tables

Adapt to existing schema names if already present.

### `document_sources`

Required columns:

- `id uuid primary key`
- `room_id uuid not null`
- `source_kind text not null`
- `title text not null`
- `license_status text not null check (...)`
- `license_reason text null`
- `visibility_default text not null`
- `created_by uuid not null`
- `metadata jsonb not null default '{}'`
- `created_at timestamptz not null default now()`
- `updated_at timestamptz not null default now()`

### `documents`

Required columns:

- `id uuid primary key`
- `source_id uuid not null references document_sources(id)`
- `room_id uuid not null`
- `title text not null`
- `normalized_hash text not null`
- `license_status text not null`
- `visibility text not null`
- `provider_metadata jsonb not null default '{}'`
- `created_at timestamptz not null default now()`

### `chunks`

Required columns:

- `id uuid primary key`
- `document_id uuid not null references documents(id)`
- `source_id uuid not null references document_sources(id)`
- `room_id uuid not null`
- `ordinal int not null`
- `heading_path text[] not null default '{}'`
- `content text not null`
- `content_hash text not null`
- `visibility text not null`
- `token_estimate int not null`
- `embedding vector(...) null` if pgvector is used
- `citation jsonb not null`
- `created_at timestamptz not null default now()`

### `ingest_jobs`

Required columns:

- `id uuid primary key`
- `room_id uuid not null`
- `source_id uuid null`
- `document_id uuid null`
- `idempotency_key text not null`
- `request_hash text not null`
- `status text not null check (...)`
- `error_code text null`
- `error_message text null`
- `chunk_count int not null default 0`
- `provider_metadata jsonb not null default '{}'`
- `response_json jsonb null`
- `created_by uuid not null`
- `created_at timestamptz not null default now()`
- `updated_at timestamptz not null default now()`

Unique index: `(room_id, created_by, idempotency_key)`.

## RLS policy requirements

Ordinary select policy:

- Actor must be room member or otherwise explicitly allowed.
- `license_status = 'allowed'` for sources/documents/chunks.
- Visibility must be allowed for actor role.
- `SystemInternal` is never returned by normal retrieval.

Review policy:

- Pending review sources are visible only through a review context, for KP/Owner/AssistantKp.
- Denied content requires a separate audit/admin path; not normal retrieval.

Recommended context keys:

```sql
app.current_user_id
app.current_room_id
app.current_room_role
app.rag_access_path -- ordinary | license_review
```

Use existing project context naming if already established.

## Repository contracts

`storage` should expose one RAG repository surface. Names may vary, but behavior must match:

```rust
#[async_trait]
pub trait RagRepository {
    async fn create_ingest_job_idempotent(...) -> Result<IdempotentOutcome<IngestJob>>;
    async fn create_source_and_document_with_chunks(...) -> Result<IngestSummary>;
    async fn list_pending_sources_for_review(...) -> Result<Vec<DocumentSource>>;
    async fn review_source(...) -> Result<DocumentSource>;
    async fn retrieve_chunks(...) -> Result<Vec<ScoredChunk>>;
}
```

All ingest writes must be transactionally consistent:

1. Claim idempotency/job row.
2. Validate license and visibility.
3. Write source/document/chunks only when allowed.
4. Store completed response only after all writes succeed.
5. Roll back failed writes.

## Required indexes

- `document_sources(room_id, license_status)`
- `documents(room_id, source_id, license_status)`
- `chunks(room_id, document_id, visibility)`
- `chunks(room_id, source_id)`
- vector index if pgvector is used
- `ingest_jobs(room_id, created_by, idempotency_key)` unique

## Direct DB tests

At least one test must use the app DB role and prove:

- Member of room A cannot read room B chunks.
- PL cannot read KP-only chunks.
- Ordinary retrieval cannot read pending/denied chunks.
- Review context allows KP/Owner to list pending sources.
- Review context does not allow PL to list pending sources.
