# Codex Batch 04 — Server API, Retrieval, and OpenAPI

Start only after Batch 03 is green.

## Read first

- `docs/p2/04_SERVER_API_OPENAPI_SPEC.md`
- `docs/p2/06_SECURITY_LEGAL_PROVIDER_POLICY.md`
- `docs/p2/07_ACCEPTANCE_TEST_MATRIX.md`
- existing `schemas/openapi.json`
- server route patterns in `crates/server`

## Tasks

1. Add minimal P2 endpoints:
   - ingest document
   - get document metadata
   - RAG query
   - list pending sources for review
   - review source
2. Use DTOs and do not leak raw DB rows.
3. Apply auth/ABAC and rely on storage/RLS for DB enforcement.
4. Enforce bounds: text size, chunk size, top_k.
5. Update `schemas/openapi.json`.
6. Add route-contract tests and negative security tests.
7. Keep P2 handlers modular; avoid growing a monolithic `server/src/lib.rs` unnecessarily.

## Constraints

- No frontend implementation in this batch.
- Retrieval returns evidence, not generated final answers.
- Errors must not reveal hidden content existence.

## Checks

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p server
cargo test --workspace
cargo sqlx prepare --check --workspace
```

## Completion response

List routes, DTOs, OpenAPI changes, and security tests.
