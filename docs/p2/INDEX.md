# P2 Execution Index

P2 mainline is **Rules / RAG / Document Ingestion**. The minimum product outcome is: a room-scoped, license-gated, permission-aware RAG retrieval path that ingests allowed sources, chunks them deterministically, embeds through a provider abstraction, stores provenance, and returns citation-bearing evidence.

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

## Batch prompts

- `prompts/codex/P2_BATCH_00_FIX_GATE.md`
- `prompts/codex/P2_BATCH_01_DOMAIN_AND_SCHEMA.md`
- `prompts/codex/P2_BATCH_02_STORAGE_RLS.md`
- `prompts/codex/P2_BATCH_03_INGEST_WORKER.md`
- `prompts/codex/P2_BATCH_04_RETRIEVAL_API.md`
- `prompts/codex/P2_BATCH_05_FRONTEND_UI.md`
- `prompts/codex/P2_BATCH_06_HARDENING_DOCS.md`
- `prompts/codex/P2_CHECK_COMMANDS.md`

## Precedence

When documents conflict:

1. Security/legal/privacy policy wins.
2. `00_P1_5_FIX_GATE.md` wins until P1.5 gate passes.
3. `01_P2_MASTER_SPEC.md` defines P2 scope.
4. Batch prompt defines implementation order, not product scope.
5. Existing historical `docs/P2_*` documents are design inputs; this folder is execution control.
