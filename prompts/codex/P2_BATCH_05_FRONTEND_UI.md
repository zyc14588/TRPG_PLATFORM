# Codex Batch 05 — Server API and OpenAPI

Compatibility note: this file keeps its legacy name for existing links. It is the B05 Server API prompt. Frontend UI moved to B06.

Start only after Batch 04 is green.

## Read first

- `CODEX_P2_MASTER_PROMPT.md`
- `docs/p2/INDEX.md`
- `docs/p2/00_EXECUTION_RULES.md`
- `docs/p2/02_BATCH_PLAN.md`
- `docs/p2/07_SERVER_API_OPENAPI.md`
- `schemas/openapi.json`
- server route patterns in `crates/server`

## Tasks

1. Add minimal P2 endpoints for ingest, document metadata, RAG evidence query, pending review list, and review decision.
2. Use typed request/response DTOs and do not leak raw DB rows.
3. Apply auth/ABAC and rely on storage/RLS for DB enforcement.
4. Enforce bounds for text size, chunk size, upload size, and `top_k`.
5. Update `schemas/openapi.json`.
6. Add route-contract tests and negative security tests.

## Constraints

- No frontend page/component implementation in this batch.
- Retrieval returns evidence, not generated final answers.
- Errors must not reveal hidden content existence.

## Checks

```powershell
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p server
cargo test --workspace
cargo sqlx prepare --check --workspace
```

## Completion response

List routes, DTOs, OpenAPI changes, and security tests.
