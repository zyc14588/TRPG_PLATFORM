# P1 Audit — Corrected Phase 1 Completion Review

Date: 2026-06-26

## Verdict

P1 is a valid foundation delivery, but it is **not yet safe to proceed directly to P2 feature implementation**. P1.5 stabilization is required first.

The repository contains the expected monorepo skeleton, core architecture documents, Auth/Room REST flow, ABAC/RLS work, idempotency primitives, provider-boundary crates, OpenAPI contract, and frontend API client. However, several P1 claims and implementation boundaries are not yet reliable enough for P2 Codex work.

## Passed Checks

- Product/backend/UI design documents exist.
- Rust workspace includes the expected crates: `server`, `auth`, `storage`, `game_core`, `rag_core`, `document_ingestor`, `llm_client`, `media_provider`, rule-system crates, agent crates, and `worker`.
- P1 Auth/Room REST handlers exist.
- Application-level ABAC and database RLS policies exist.
- Frontend room DTO parsing rejects KP-only fields.
- Development auth is gated behind explicit development mode.

## Corrected Gaps

- `rag_core` is still a skeleton. It does not yet implement the full P2 RAG kernel.
- `document_ingestor` currently defines its own license status model instead of delegating to `rag_core`.
- `prompts/03_RULES_RAG.md` now names the active P2 track as Rules/RAG/Document Ingestion, but it is only a target prompt until the P1.5 gate passes.
- `prompts/02_REALTIME_CONCURRENCY.md` is deferred to Phase 3/P2B and is not the current P2 mainline.
- Idempotency is claimed before some business writes and must be moved into repository transactions.
- Invitation accept duplicate replay is not reliable after the invite leaves `pending` state.
- Production startup must not fall back to in-memory storage when `DATABASE_URL` is missing.
- Auth-private-table RLS strategy must be clarified for non-superuser production DB roles.
- RAG license-status RLS must enforce `allowed` on normal retrieval paths.

## Required P1.5 Fixes Before P2

1. Correct P1/P2 documentation and phase numbering.
2. Harden production startup and secret validation.
3. Move idempotency and business writes into single transactions.
4. Make refresh session rotation atomic.
5. Resolve auth-private-table RLS access model.
6. Enforce RAG license-status at DB and retrieval layers.
7. Clean release packaging so source archives exclude `.git`, `node_modules`, build outputs, and generated caches.

## P2 Gate

P2 Rules/RAG/Ingestion may start only after the P1.5 stabilization checklist passes with tests.
