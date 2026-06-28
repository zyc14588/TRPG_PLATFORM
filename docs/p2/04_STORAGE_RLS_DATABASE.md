# B02 Spec — Storage, PostgreSQL, RLS, Database

## Goal

Implement the persistence and enforcement layer for P2. This batch must prove that room boundary, role boundary, license boundary, visibility boundary and idempotency are enforced at repository/DB level, not only in API/frontend code.

## Migration rule

Migrations must be additive. Do not rewrite existing migrations that may already have been applied. If a CHECK constraint is wrong, add a new migration that drops/recreates the constraint safely.

## Required tables or equivalents

### `document_sources`

```sql
id uuid primary key,
room_id uuid not null,
source_kind text not null,
title text not null,
license_status text not null,
license_reason text null,
visibility_default text not null,
created_by uuid not null,
metadata jsonb not null default '{}',
created_at timestamptz not null default now(),
updated_at timestamptz not null default now()
```

### `documents`

```sql
id uuid primary key,
source_id uuid not null references document_sources(id),
room_id uuid not null,
title text not null,
normalized_hash text not null,
license_status text not null,
visibility text not null,
provider_metadata jsonb not null default '{}',
created_at timestamptz not null default now()
```

### `chunks`

```sql
id uuid primary key,
document_id uuid not null references documents(id),
source_id uuid not null references document_sources(id),
room_id uuid not null,
ordinal int not null,
heading_path text[] not null default '{}',
content text not null,
content_hash text not null,
visibility text not null,
token_estimate int not null,
embedding vector(...) null,
citation jsonb not null,
created_at timestamptz not null default now()
```

If pgvector is not used, document the equivalent vector storage strategy.

### `ingest_jobs`

```sql
id uuid primary key,
room_id uuid not null,
source_id uuid null,
document_id uuid null,
idempotency_key text not null,
request_hash text not null,
status text not null,
error_code text null,
error_message text null,
chunk_count int not null default 0,
provider_metadata jsonb not null default '{}',
response_json jsonb null,
created_by uuid not null,
created_at timestamptz not null default now(),
updated_at timestamptz not null default now()
```

Unique index:

```sql
unique (room_id, created_by, idempotency_key)
```

If project intentionally uses room-scoped idempotency without `created_by`, document why two actors cannot conflict.

## Required CHECK constraints

All serialized enum values used by Rust must be accepted by DB constraints, and DB constraints must reject unknown values. At minimum:

```text
license_status: allowed, pending_review, denied
visibility: public_rule, room_rule, player_visible_clue, character_private, kp_only_module, kp_secret, memory_private, system_internal
ingest_jobs.status: queued/claimed, parsing, embedding, indexed/completed, pending_review, denied, failed
```

Missing `denied` in `ingest_jobs.status` is a blocker.

## RLS context

Prefer existing project functions/settings. If absent, establish a consistent context such as:

```text
app.current_user_id
app.current_room_id
app.current_room_role
app.current_rag_access_path = ordinary | license_review | admin_maintenance
```

Rules:

- ordinary retrieval path cannot select pending/denied chunks.
- review path can list pending sources only for KP/Owner/AssistantKP or project equivalent.
- cross-room read/write is denied.
- SystemInternal never appears in ordinary retrieval.
- RLS must be enabled and forced on P2 tables unless a documented exception exists.

## Repository contracts

Repository should expose domain/DTO types, not raw SQL rows.

Required methods or equivalents:

- `create_ingest_job_idempotent`
- `create_document_source`
- `create_document_with_chunks`
- `update_ingest_job_status`
- `list_pending_sources_for_review`
- `review_source`
- `retrieve_candidate_chunks`
- `get_document_metadata`

Required error mapping:

- not found
- forbidden/hidden
- conflict
- validation
- DB unavailable
- policy violation

## Retrieval prefilter

Filtering must occur before scoring. Implementation may use SQL predicates, RLS policies, materialized authorized candidate views, or a repository function that is impossible to call without actor/room context. The scored candidate set must exclude denied, pending, cross-room and invisible rows.

## Idempotency semantics

For ingest:

- same `(room_id, created_by, idempotency_key)` + same request hash returns replay response.
- same key + different request hash returns conflict.
- incomplete/failed jobs follow documented retry semantics.
- repeated chunk persistence must not duplicate searchable data.

## Required tests

- `denied_ingest_job_status_is_persistable`
- `rls_blocks_pending_denied_chunks`
- `rls_blocks_cross_room_chunks`
- `public_rule_requires_allowed_license`
- `pl_cannot_retrieve_kp_only_module`
- `observer_cannot_retrieve_character_private`
- `kp_can_retrieve_kp_only`
- `system_internal_never_returns`
- `review_path_lists_pending_for_kp`
- `pl_cannot_review_sources`
- `retrieval_filters_before_scoring`
- `ingest_duplicate_replays`
- `ingest_conflict_on_hash_mismatch`

## DB setup link

See `11_DATABASE_SETUP.md` for local roles, migrations and SQLx workflow.

## Batch boundary

B02 must not add public HTTP endpoints, frontend pages or Rig runtime. It may expose Rust repository traits/impls used by later batches.
