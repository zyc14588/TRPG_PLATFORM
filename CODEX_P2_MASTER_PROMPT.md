# CODEX P2 MASTER PROMPT - TRPG_PLATFORM

You are working in the TRPG_PLATFORM repository. Your mission is Phase 2: Rules / RAG / Document Ingestion, but P2 implementation may start only after the P1.5 Fix Gate passes.

This is the global control prompt for future P2 batches. The P2 Master Preparation batch is docs-only: it may update planning and check-command documents, but it must not implement P2 runtime features.

## P2 mission

Deliver a secure RAG retrieval foundation for TRPG rooms. The system must ingest allowed text sources, chunk them deterministically, embed through a provider abstraction, store provenance, apply license and visibility filtering before scoring, and return citation-bearing evidence.

## Non-goals

- Do not import unauthorized commercial rule prose.
- Do not require full PDF/OCR in P2. Text and Markdown ingestion is enough.
- Do not implement final LLM answer generation in P2. Retrieval returns evidence only.
- Do not make WebSocket, Redis, or outbox replay part of the P2 mainline.
- Do not make CI tests call real cloud providers.
- Do not treat frontend UI as an access-control boundary.

## Mandatory reading order

Read these files before modifying P2 code:

1. `README.md`
2. `AGENTS.md` if present
3. `CODEX_MASTER_PROMPT.md` if present
4. `docs/P1_5_FIX_PLAN.md`
5. `docs/p2/00_P1_5_FIX_GATE.md`
6. `docs/SECURITY_RLS_POLICY.md`
7. `docs/LEGAL_POLICY.md`
8. `docs/RAG_DESIGN.md`
9. `docs/P2_CODEX_HANDOFF.md`
10. `docs/P2_RAG_IMPLEMENTATION_SPEC.md`
11. `docs/P2_RAG_ACCEPTANCE_TESTS.md`
12. `docs/p2/INDEX.md`
13. `docs/p2/01_P2_MASTER_SPEC.md`
14. `docs/p2/02_RAG_CORE_DOMAIN_SPEC.md`
15. `docs/p2/03_STORAGE_RLS_AND_MIGRATIONS.md`
16. `docs/p2/04_SERVER_API_OPENAPI_SPEC.md`
17. `docs/p2/05_FRONTEND_RAG_UI_SPEC.md`
18. `docs/p2/06_SECURITY_LEGAL_PROVIDER_POLICY.md`
19. `docs/p2/07_ACCEPTANCE_TEST_MATRIX.md`
20. The current batch prompt in `prompts/codex/`
21. `prompts/codex/P2_CHECK_COMMANDS.md`

When code changes are planned, also read the relevant current implementation files under `crates/rag_core`, `crates/document_ingestor`, `crates/storage`, `crates/server`, `apps/web`, `schemas/openapi.json`, and the workspace `Cargo.toml`.

The existing `docs/CODEX_P2_MASTER_PROMPT.md` is a historical design-control input. Keep it unless the owner explicitly asks to remove or move it.

## Global constraints

- P1.5 Fix Gate must be green before P2 implementation starts.
- License gate must happen before chunking, embedding, indexing, and retrieval.
- Visibility filter must happen before scoring, ranking, or reranking.
- `pending_review` and `denied` content must not enter ordinary chunking, embedding, indexing, retrieval, normal API responses, or ordinary DB-role reads.
- Every retrieval result must include `source_id`, `document_id`, `chunk_id`, `content_hash`, `citation`, and `visibility` metadata.
- Bound `top_k`, upload size, raw text size, and chunk size.
- Ingestion must be idempotent: same key plus same payload replays the response; same key plus different payload conflicts.
- LocalOnly rooms must reject cloud providers and use deterministic local test providers.
- API responses must be DTOs, not raw DB rows.
- Do not add unauthorized commercial rule text, fixtures, screenshots, generated examples, or copied passages.
- Do not use the `postgres` superuser as a production application login role.
- Do not bypass ABAC/RLS with application comments or UI-only checks.

## Batch discipline

Work in order:

0. P2 Master Preparation - docs/control only; no runtime feature work
1. P1.5 Fix Gate
2. Domain
3. Storage/RLS
4. Ingest Worker
5. Server API
6. Frontend UI
7. Hardening

A batch may not start until the previous batch gate passes. If a batch discovers a blocker, fix it in the same batch only when it is required for that batch. Otherwise document the blocker and stop.

This master prompt is a control document. It does not itself mark P2 complete.

## Definition of P2 done

P2 is complete only when all of the following are true:

- `cargo fmt --all --check` passes.
- `cargo check --workspace` passes.
- `cargo clippy --workspace --all-targets -- -D warnings` passes.
- `cargo test --workspace` passes.
- SQLx migrations run on a fresh database.
- `cargo sqlx prepare --check --workspace` passes.
- `pnpm install --frozen-lockfile`, `pnpm lint`, `pnpm typecheck`, `pnpm test`, `pnpm test:e2e`, and `pnpm build` pass.
- Retrieval returns evidence/citation/provenance, not final generated answers.
- Direct DB role tests prove normal retrieval cannot read pending/denied or invisible chunks.
- LocalOnly mode never invokes cloud model, embedding, rerank, OCR, or image providers.
- Every row in `docs/p2/07_ACCEPTANCE_TEST_MATRIX.md` is implemented with a passing test or explicitly deferred with owner-approved rationale in `docs/status/P2_STATUS.md`.
- `docs/status/P2_STATUS.md` records exact commands and results.
