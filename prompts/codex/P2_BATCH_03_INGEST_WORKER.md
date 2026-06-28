# Codex Batch 03 — Document Ingestor and Worker Orchestration

Start only after Batch 02 is green.

## Read first

- `CODEX_P2_MASTER_PROMPT.md`
- `docs/p2/INDEX.md`
- `docs/p2/00_EXECUTION_RULES.md`
- `docs/p2/02_BATCH_PLAN.md`
- `docs/p2/05_INGEST_WORKER.md`
- `docs/OUTBOX_AND_RETRY.md` if present

## Tasks

1. Implement document_ingestor orchestration:
   - validate source metadata
   - check license
   - reject/pending before chunking when not allowed
   - chunk allowed text
   - embed using provider trait
   - persist source/document/chunks/job summary through storage
2. Add deterministic no-network ingest smoke test.
3. Record provider metadata and chunk/citation provenance.
4. Handle idempotent replay/conflict.
5. If worker crate is present, add a minimal job runner boundary without requiring Redis/outbox replay.

## Constraints

- No real cloud providers in tests.
- No server route or frontend implementation in this batch.
- Do not index pending/denied sources for convenience.

## Checks

```powershell
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p rag_core -p document_ingestor -p storage
cargo test --workspace
```

## Completion response

List ingest path, tests, provider metadata, and idempotency behavior.
