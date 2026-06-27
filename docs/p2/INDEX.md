# P2 Execution Index

P2 mainline is **Rules / RAG / Document Ingestion**. The minimum product outcome is a room-scoped, license-gated, permission-aware RAG retrieval path that ingests allowed text sources, chunks them deterministically, embeds through a provider abstraction, stores provenance, applies license and visibility filtering before scoring, and returns citation-bearing evidence.

This folder is execution control for P2. Historical design inputs in `docs/P2_*`, `docs/RAG_DESIGN.md`, and `docs/03_RULES_RAG_EXPANDED.md` must be preserved and used as context, but this index defines the working order for future P2 batches.

## Hard gate

Do not start P2 implementation until the P1.5 Fix Gate is green. `docs/p2/00_P1_5_FIX_GATE.md` and `docs/P1_5_FIX_PLAN.md` take precedence over all P2 implementation prompts until the gate has passed.

This preparation batch does not mark P2 complete and does not implement runtime RAG behavior.

Windows Codex App 默认使用 PowerShell。所有可复制执行的命令块必须使用 `powershell` fence。避免 Bash-only 的错误吞咽、条件链、pipefail 初始化、POSIX 递归建目录参数、Bash 环境变量导出写法和 Unix 临时目录路径；改用 `$LASTEXITCODE`、`New-Item -Force`、`$env:VAR = "value"` 和 `$env:TEMP` 等 PowerShell 形式。

## Reading order

Read in this order before any P2 implementation batch:

1. `README.md`
2. `AGENTS.md`
3. `CODEX_MASTER_PROMPT.md`
4. `CODEX_P2_MASTER_PROMPT.md`
5. `docs/P1_5_FIX_PLAN.md`
6. `docs/p2/00_P1_5_FIX_GATE.md`
7. `docs/SECURITY_RLS_POLICY.md`
8. `docs/LEGAL_POLICY.md`
9. `docs/RAG_DESIGN.md`
10. `docs/P2_CODEX_HANDOFF.md`
11. `docs/P2_RAG_IMPLEMENTATION_SPEC.md`
12. `docs/P2_RAG_ACCEPTANCE_TESTS.md`
13. `docs/p2/01_P2_MASTER_SPEC.md`
14. `docs/p2/02_RAG_CORE_DOMAIN_SPEC.md`
15. `docs/p2/03_STORAGE_RLS_AND_MIGRATIONS.md`
16. `docs/p2/04_SERVER_API_OPENAPI_SPEC.md`
17. `docs/p2/05_FRONTEND_RAG_UI_SPEC.md`
18. `docs/p2/06_SECURITY_LEGAL_PROVIDER_POLICY.md`
19. `docs/p2/07_ACCEPTANCE_TEST_MATRIX.md`
20. The current batch prompt in `prompts/codex/`
21. `prompts/codex/P2_CHECK_COMMANDS.md`

When code changes are planned, also inspect the active surfaces before editing:

- `schemas/openapi.json`
- workspace `Cargo.toml`
- `crates/rag_core/**`
- `crates/document_ingestor/**`
- `crates/storage/**`
- `crates/server/**`
- `apps/web/**`

## Preparation batch

Current batch: P2 Master Preparation.

Scope: docs and control files only. This batch may create or update `docs/p2/INDEX.md`, `CODEX_P2_MASTER_PROMPT.md`, `prompts/codex/P2_CHECK_COMMANDS.md`, and status/documentation directories only when needed for batch discipline. It must not implement domain, storage, API, worker, or frontend runtime RAG behavior.

Exit: the repository has a clear P2 reading order, a batch plan, Windows-friendly check commands, and an explicit statement that P2 implementation remains blocked until the P1.5 Fix Gate is green.

Next batch: Batch 00 - P1.5 Fix Gate.

## Batch plan

### Batch 00 - P1.5 Fix Gate

Prompt: `prompts/codex/P2_BATCH_00_FIX_GATE.md`

Goal: close or explicitly verify P1.5 blockers before P2 implementation starts. This includes production startup safety, idempotency transaction boundaries, refresh rotation atomicity, private auth table access strategy, RLS/license enforcement, health/readiness/metrics, generated artifact hygiene, and frontend phase-label cleanup.

Exit: all required gate checks pass, or the batch stops with a precise blocker report. P2 implementation remains blocked until this exit is green.

### Batch 01 - Domain

Prompt: `prompts/codex/P2_BATCH_01_DOMAIN_AND_SCHEMA.md`

Goal: normalize `rag_core` ownership of RAG domain semantics, including source, document, chunk, citation, license, visibility, retrieval query/result, chunking, embedding, vector store, keyword index, and hybrid retriever contracts. `document_ingestor` must not own a second incompatible `LicenseStatus`.

Exit: domain types and deterministic local test contracts compile, with focused tests for license decisions, chunk hashes, top_k bounds, citation fields, and provider privacy classification.

### Batch 02 - Storage/RLS

Prompt: `prompts/codex/P2_BATCH_02_STORAGE_RLS.md`

Goal: add only additive migrations and repository contracts needed for P2 RAG storage. Enforce license and visibility in ordinary retrieval policies and direct DB role tests.

Exit: normal DB roles cannot read cross-room, pending_review, denied, KP-only, private, or SystemInternal rows through ordinary retrieval paths. Pending review access is only through the explicit review context.

### Batch 03 - Ingest Worker

Prompt: `prompts/codex/P2_BATCH_03_INGEST_WORKER.md`

Goal: implement deterministic text/Markdown ingestion orchestration after the gate passes: license check, bounded normalization, chunking, hashing, embedding, index/store writes, provider metadata, job state, and idempotent replay/conflict behavior.

Exit: allowed text can ingest and retrieve in local deterministic tests; pending_review and denied content is not chunked, embedded, indexed, or retrievable.

### Batch 04 - Server API

Prompt: `prompts/codex/P2_BATCH_04_RETRIEVAL_API.md`

Goal: expose minimal DTO-based API for document ingest, document view, RAG query, pending review list, and review action. Update `schemas/openapi.json` and route-contract tests with no raw DB row exposure.

Exit: API tests prove bearer/CSRF expectations, top_k and upload bounds, idempotency replay/conflict, LocalOnly provider rejection, and citation-bearing evidence responses.

### Batch 05 - Frontend UI

Prompt: `prompts/codex/P2_BATCH_05_FRONTEND_UI.md`

Goal: add the minimal KP/Owner management flow for paste/upload text, pending review, RAG query, and evidence/citation display. The UI is not an access-control boundary.

Exit: frontend client DTO tests reject hidden KP-only fields, PL review controls are absent, and evidence renders source/title/heading/content hash/citation metadata.

### Batch 06 - Hardening

Prompt: `prompts/codex/P2_BATCH_06_HARDENING_DOCS.md`

Goal: run full P2 verification, close negative tests, verify performance bounds, update status, and document exact command results.

Exit: every row in `docs/p2/07_ACCEPTANCE_TEST_MATRIX.md` is implemented with passing tests or explicitly deferred with owner-approved rationale in `docs/status/P2_STATUS.md`.

## Required invariants

1. License gate runs before chunking, embedding, indexing, and retrieval.
2. Visibility filtering runs before scoring, ranking, or reranking.
3. `pending_review` and `denied` content cannot enter ordinary retrieval by API or direct DB role.
4. Every retrieval result includes `source_id`, `document_id`, `chunk_id`, `content_hash`, `citation`, and `visibility` metadata.
5. `top_k`, upload size, raw text size, and chunk size are bounded.
6. Ingestion is idempotent: same key plus same payload replays the response; same key plus different payload conflicts.
7. LocalOnly rooms reject cloud providers and use deterministic local test providers.
8. API returns DTOs, not raw DB rows.

## Documents

- `00_P1_5_FIX_GATE.md`: mandatory pre-P2 gate and known closure checks.
- `01_P2_MASTER_SPEC.md`: scope, non-goals, architecture, batch order.
- `02_RAG_CORE_DOMAIN_SPEC.md`: domain model, traits, chunking, embeddings, evidence.
- `03_STORAGE_RLS_AND_MIGRATIONS.md`: tables, indexes, RLS, repository contracts.
- `04_SERVER_API_OPENAPI_SPEC.md`: endpoints, DTOs, status codes, API tests.
- `05_FRONTEND_RAG_UI_SPEC.md`: minimal UX and frontend contracts.
- `06_SECURITY_LEGAL_PROVIDER_POLICY.md`: license, privacy mode, provider boundaries.
- `07_ACCEPTANCE_TEST_MATRIX.md`: required tests by layer.
- `08_STATUS_REPORT_TEMPLATE.md`: final P2 status report template.

## Precedence

When documents conflict:

1. Security, legal, and privacy policy wins.
2. `00_P1_5_FIX_GATE.md` wins until P1.5 gate passes.
3. `CODEX_P2_MASTER_PROMPT.md` and `01_P2_MASTER_SPEC.md` define active P2 scope.
4. Batch prompts define implementation order, not product scope.
5. Existing historical `docs/P2_*` documents are design inputs; this folder is execution control.
