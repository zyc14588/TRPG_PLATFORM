# P2 Batch Plan

## B00 — Docs install / Prep gate

Purpose: install this persistent documentation set and verify reading order.

Allowed changes:

- `CODEX_P2_MASTER_PROMPT.md`
- `.codex/P2_SESSION_START.md`
- `docs/p2/**`
- `docs/status/P2_STATUS_TEMPLATE.md`
- `prompts/codex/P2_CHECK_COMMANDS.md`
- README/AGENTS reading-order references only

Forbidden:

- runtime RAG code
- DB migrations
- server routes
- frontend pages
- dependency upgrades

Exit criteria:

- Codex can find P2 docs from repo root.
- Batch boundaries and acceptance matrix are present.
- No runtime implementation was added.

## B01 — RAG Core Domain Contracts

Purpose: build stable P2 domain model and deterministic test providers.

Allowed:

- `crates/rag_core/**`
- tests for domain/chunk/hash/provider traits
- small compatibility changes in `crates/document_ingestor` only if needed to compile against shared types

Forbidden:

- DB migrations
- public server routes
- frontend pages
- live provider calls

Exit criteria:

- License/visibility/privacy/status types exist.
- Chunk/hash/citation contracts are deterministic.
- Provider traits are provider-agnostic and no-network in tests.

## B02 — Storage, PostgreSQL, RLS, Database

Purpose: implement persistent schema, repository contracts, RLS, idempotency and database setup.

Allowed:

- additive migrations
- `crates/storage/**`
- SQLx/RLS tests
- DB setup docs/scripts

Forbidden:

- public HTTP endpoints
- frontend UI
- worker orchestration beyond compile-only interfaces
- direct Rig integration

Exit criteria:

- room/role/license/visibility enforced by repository/RLS.
- `pending_review` and `denied` are DB-deny-by-default for ordinary retrieval.
- idempotency replay/conflict tested.
- Rust enum/DTO status values and DB CHECK constraints are aligned.

## B03 — Document Ingestor and Worker

Purpose: implement deterministic, license-first ingestion orchestration and optional worker runner.

Allowed:

- `crates/document_ingestor/**`
- `crates/worker/**` if present or added intentionally
- integration with storage repository
- deterministic provider stubs

Forbidden:

- public HTTP routes
- frontend UI
- live cloud provider tests
- final answer generation

Exit criteria:

- pending/denied not chunked/indexed.
- allowed text indexed with provenance.
- failed jobs not completed.
- repeated ingest is replay/conflict safe.

## B04 — Rig Agent Engine

Purpose: add `crates/agent_engine` or equivalent Rig-backed orchestration layer.

Allowed:

- Rig dependency integration with feature gates
- provider registry and privacy policy enforcement
- retrieval tools that call policy-guarded repository/service
- deterministic fake/local agent tests

Forbidden:

- bypassing storage/RLS
- final answer UX
- frontend pages
- client-supplied provider secrets

Exit criteria:

- LocalOnly blocks cloud providers.
- Rig agent/tool workflow returns evidence bundle or structured plan only.
- provider metadata recorded without secrets.

## B05 — Server API and OpenAPI

Purpose: expose minimal P2 HTTP API and synchronize schema.

Allowed:

- server DTO/routes/tests
- OpenAPI schema updates
- integration of ingestor/storage/agent_engine via service layer

Forbidden:

- frontend pages
- final generated answer as P2 response
- relying on frontend as security boundary

Exit criteria:

- ingest/query/review endpoints exist.
- auth/CSRF/room membership/RLS/license/visibility/idempotency negative tests pass.
- OpenAPI matches implementation.

## B06 — Frontend RAG UI

Purpose: minimal Next.js UI for documents, ingest, review, evidence query.

Allowed:

- typed backend client functions
- frontend pages/components/tests
- copy updates

Forbidden:

- backend semantic changes except tiny contract fixes
- secret exposure
- generated-answer chat UX

Exit criteria:

- UI shows evidence/citations/provenance.
- PL review controls absent; KP-only fields absent from PL DTO.
- CSRF/bearer behavior preserved.

## B07 — Hardening and Final Gate

Purpose: finish acceptance matrix, status docs, negative tests, and full gates.

Allowed:

- tests
- docs/status
- small bug fixes
- OpenAPI/doc sync

Forbidden:

- new large product features
- dependency churn
- weakening tests

Exit criteria:

- `docs/status/P2_STATUS.md` complete.
- full Rust/SQLx/frontend gates pass or environment blockers are honestly recorded.
- P2 final acceptance matrix has no unexplained FAIL.
