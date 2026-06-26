# P2 Delivery Plan — Rules, RAG, and Document Ingestion

Status: target plan. P2 may start only after P1.5 stabilization fixes production startup, idempotency transaction boundaries, refresh rotation atomicity, auth-private-table RLS strategy, and license-status RLS enforcement.

## Goal

Deliver the first executable Rules/RAG/Ingestion path: rule adapter boundaries, legal source gate, chunking, local embedding, local vector retrieval, visibility filtering, and citation-bearing evidence.

## In Scope

- `rag_core` domain model and traits.
- `document_ingestor` license gate alignment.
- Deterministic local embedding for tests and local-only mode.
- In-memory local vector store for smoke tests.
- Permission-first retrieval filters.
- RAG persistence schema contract for `document_sources`, `documents`, `chunks`, and `ingest_jobs`.
- Repository transaction traits and ingest job state transitions.

## Out of Scope for This Pass

- Full pgvector adapter implementation.
- Tantivy BM25 adapter implementation.
- Persistent SQLite/JSONL store implementation.
- Public API routes for ingest/search.
- Background worker ingestion execution.
- Automated web crawling of rulebooks.

These remain P2 follow-ups, not P1 blockers.

## Acceptance Criteria

- Unknown licenses are `pending_review` and not indexed.
- Commercial adapter text is denied unless explicitly adapter-only.
- Local-only ingestion rejects cloud embedders.
- Chunks have stable hashes.
- Retrieval returns citation metadata.
- PL-visible filters cannot retrieve KP-only chunks.
- Cargo tests for the new RAG kernel pass.

## Follow-up Sequence

1. Implement SQLx repositories behind the Phase 2A `RagRepositoryTransaction` contract.
2. Add server endpoints for document ingest/search.
3. Add pgvector adapter and local SQLite adapter behind `VectorStore`.
4. Add worker outbox job for asynchronous embeddings.
5. Extend OpenAPI and frontend admin screens for source review.
