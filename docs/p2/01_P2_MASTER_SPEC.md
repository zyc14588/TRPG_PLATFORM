# P2 Master Spec — Rules / RAG / Document Ingestion

## Goal

Deliver a secure RAG retrieval foundation for TRPG rooms. The system ingests allowed text sources, chunks them deterministically, embeds through a provider abstraction, stores provenance, applies license and visibility filtering before scoring, and returns citation-bearing evidence.

## Non-goals

- No unauthorized commercial rule prose.
- No full PDF/OCR requirement in P2. Text/Markdown ingestion is enough; PDF/OCR remains an adapter boundary.
- No final LLM answer generation in P2. Retrieval returns evidence only.
- No WebSocket/Redis/outbox replay in the P2 mainline.
- No real cloud provider calls in CI tests.
- No UI-only access control.

## Existing baseline

The workspace already has P2-relevant crates such as `rag_core`, `document_ingestor`, `llm_client`, `storage`, `server`, and `worker`. Existing migrations already include RAG scaffolding. P2 should extend and harden this baseline, not duplicate semantics in new parallel modules.

## Architectural layers

```text
apps/web
  -> server API DTOs/OpenAPI
    -> application handlers / ABAC
      -> storage repositories / transactions
        -> PostgreSQL tables, indexes, RLS policies
      -> document_ingestor orchestration
        -> rag_core traits/domain
          -> chunker, embedder, vector store, keyword index, hybrid retriever
```

## Required invariants

1. License gate runs before chunking, embedding, indexing, and retrieval.
2. Visibility filter runs before scoring/ranking/reranking.
3. `pending_review` and `denied` content cannot enter ordinary retrieval by API or direct DB role.
4. Every retrieval result has `source_id`, `document_id`, `chunk_id`, `content_hash`, `citation`, and `visibility` metadata.
5. `top_k`, upload size, raw text size, and chunk size are bounded.
6. Ingestion is idempotent: same key + same payload replays the response; same key + different payload conflicts.
7. LocalOnly rooms reject cloud providers and use deterministic local test providers.
8. API returns DTOs, not raw DB rows.

## Batch plan

### Batch 00 — P1.5 Fix Gate

Close boot, license, health, artifact, UI, dependency, CSRF, and module-boundary gaps.

### Batch 01 — Domain and schema contracts

Implement or normalize `rag_core` types and traits. Add JSON/OpenAPI schema stubs where useful. Ensure `document_ingestor` imports license/status types from `rag_core` rather than defining its own.

### Batch 02 — Storage/RLS

Add additive migrations for missing columns/policies/indexes. Implement repository contracts and direct DB role tests.

### Batch 03 — Ingest worker

Implement deterministic text ingestion: normalize, license-check, chunk, hash, embed, index/store, record job state and provider metadata.

### Batch 04 — Retrieval API

Implement minimal server routes for ingest, source review, document view, and RAG query. Update OpenAPI and route tests.

### Batch 05 — Frontend UI

Implement minimal admin flow: paste/upload text, pending review, run query, show evidence/citations. Test DTO privacy.

### Batch 06 — Hardening and docs

Performance bounds, negative tests, status report, full CI gate, and documentation update.

## Completion definition

P2 is complete when every row in `docs/p2/07_ACCEPTANCE_TEST_MATRIX.md` is either implemented with a passing test or explicitly deferred with owner-approved rationale in `docs/status/P2_STATUS.md`.
