# P1 Audit — Phase 1 Completion Review

Date: 2026-06-26
Baseline commit reviewed: `03bec7b p1 done`

## Verdict

P1 is a valid foundation delivery, but it is **not safe to proceed directly to P2 feature implementation**. P1.5 stabilization must pass first.

The repository contains the required monorepo skeleton, core architecture decisions, ABAC/RLS migrations, idempotency primitives, optimistic-write types, SQLSTATE retry classification, provider abstraction crates, and phase status reports. It does not yet contain an executable P2 RAG kernel.

## Passed Checks

- `DECISIONS.md`, `docs/PRODUCT_SYSTEM_DESIGN.md`, `docs/BACKEND_ARCHITECTURE.md`, and `docs/UI_UX_SPEC.md` exist.
- Rust workspace contains the expected core crates: `server`, `auth`, `game_core`, `rag_core`, `document_ingestor`, `llm_client`, `media_provider`, rules crates, agent crates, storage, and worker.
- Migrations include privacy modes, document/chunk/embedding tables, outbox, idempotency keys, and RLS-related policy work.
- `auth` defines room roles, visibility scopes, privacy modes, and local-only cloud-provider denial tests.
- `game_core` defines `expected_version`, `idempotency_key`, and retry classification for `40P01`, `40001`, and `55P03`.
- `llm_client` and `media_provider` expose provider boundaries rather than direct feature calls from application code.
- Server tests include KP-only projection and duplicate idempotency replay coverage.

## Gaps Found

- `rag_core` is still a trait/type skeleton. It lacks P2 domain objects such as `DocumentSource`, `DocumentMetadata`, `LicenseStatus`, `Chunk`, `Chunker`, `Embedder`, `RetrievalQuery`, `RetrievalResult`, and an executable local store.
- `document_ingestor` contains a minimal license helper, but its status model is separate from `rag_core` and does not encode commercial adapter denial as a first-class test.
- P2 design and prompt files are target instructions only; they do not mean the P2 RAG kernel has been implemented.
- `prompts/03_RULES_RAG.md` now defines the P2 boundary, stop points, prohibited work, and acceptance criteria, but implementation must wait for the P1.5 gate.
- Worker currently has no background ingest jobs enabled. This is acceptable for P1 but remains a P2/P3 follow-up.
- Phase policy: Rules/RAG/Document Ingestion is the only mainline P2 definition; Realtime/WebSocket/Redis/Outbox Replay is deferred to Phase 3/P2B.

## P1.5 Gate Before P2

P2 Rules/RAG/Ingestion may start only after P1.5 fixes and tests are complete:

- production startup cannot silently fall back to in-memory storage;
- auth configuration rejects unsafe production secrets and cookie settings;
- room command idempotency is transactional;
- invitation acceptance can replay successful idempotent retries;
- refresh token rotation is atomic;
- auth-private-table RLS has a production-safe access model;
- RAG license-status RLS blocks pending/denied content from normal retrieval paths;
- source packaging excludes `.git`, `node_modules`, build output, and generated caches.

## Remaining P2 Follow-ups

- Implement the P2 RAG kernel in `crates/rag_core`.
- Move `document_ingestor` license semantics to `rag_core`.
- Add persistent pgvector/Tantivy/SQLite adapters behind the `VectorStore` boundary.
- Add SQLx-backed ingestion repository and pending-review workflow.
- Wire server endpoints to the RAG kernel with room-scoped authorization.
- Add OpenAPI entries for document ingest/search endpoints when server routes are implemented.
