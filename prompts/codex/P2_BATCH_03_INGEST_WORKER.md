# Codex Batch 03 — Document Ingestor and Worker Orchestration

Start only after Batch 02 is green.

## Read first

- `docs/p2/02_RAG_CORE_DOMAIN_SPEC.md`
- `docs/p2/03_STORAGE_RLS_AND_MIGRATIONS.md`
- `docs/p2/06_SECURITY_LEGAL_PROVIDER_POLICY.md`
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

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p rag_core -p document_ingestor -p storage
cargo test --workspace
```

## Completion response

List ingest path, tests, provider metadata, and idempotency behavior.
