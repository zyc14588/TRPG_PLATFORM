# Source Processing Record: ADR 0005 Postgres Pgvector

Batch: BATCH-026
Prompt: P0064 / CODEX-0641-06-DATA-EVENTING-95d90eabef
Kind: documentation-or-traceability

## Current-Safe Output

- Output: docs/codex/06-data-eventing/source_processing_record_docs_adr_adr_0005_postgres_pgvector.md
- Scope: traceability record only
- Implementation output: none

## Governance Assertions

- Vector and RAG indexes are rebuildable read models sourced from Event Store history.
- RAG snapshot metadata must include source_type, visibility, version, owner, allowed_use, and fact provenance.
- RAG retrieval must not leak Keeper-only, AI-internal, or private-player facts to unauthorized principals.
- Index rebuilds must keep source event sequence and chunk hash metadata for replay and audit.

## Batch Evidence

- Work plan: evidence/batches/BATCH-026/WORK_PLAN.md
- Test record: evidence/batches/BATCH-026/TEST_RESULTS.md

