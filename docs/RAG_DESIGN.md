# RAG Design — Phase 2 Baseline

Implementation status: target design. The current P1/P1.5 codebase does not yet implement all types and local adapters listed here; P2 must add them after the P1.5 gate passes.

## Purpose

Phase 2 establishes the legal, privacy-aware retrieval kernel used by rules, modules, clues, session logs, and memory. Retrieval must return evidence and metadata; it must not answer user questions directly and must not leak KP-only data to PL clients.

## Data Model

P2 must implement these core types in `crates/rag_core`:

- `DocumentSource`: source identity, kind, URL, declared rights, and commercial-adapter marker.
- `LicenseStatus`: `allowed`, `pending_review`, or `denied`.
- `DocumentMetadata`: document type, system name, visibility scope, license, and citation source.
- `Chunk`: stable chunk unit with `content_hash`, visibility, license, and citation metadata.
- `RetrievalQuery`: user query, query embedding, and retrieval filter.
- `Evidence`: short preview plus citation and score.

## License Gate

Indexing is gated by `check_license` before chunking or embedding.

- Official SRD and recognized open-license sources may be indexed.
- User uploads may be indexed only when the user declares they have rights.
- Unknown or unclear licenses become `pending_review` and are not indexed.
- Known incompatible terms such as no redistribution or non-commercial become `denied`.
- Commercial adapters are allowed only when they contain mechanics/schema code and no commercial rule text.

## Chunking

`MarkdownChunker` supports Markdown and plain text. It preserves heading paths, enforces a bounded chunk size, and writes stable SHA-256 hashes over document ID, chunk index, and normalized text. PDF and OCR parsing remain adapter work outside `rag_core`.

## Embedding

`Embedder` is a provider boundary. The Phase 2 target local implementation is deterministic and local-only safe. Cloud embedding providers must return `is_cloud_provider() == true`, and ingestion rejects them when `PrivacyMode::LocalOnly` is active.

## Retrieval

`LocalVectorStore` is the Phase 2 target executable adapter. It applies license and visibility filtering before returning hits. Scores combine deterministic vector similarity and a small keyword score so tests can prove retrieval behavior without cloud calls.

Production adapters must preserve the same semantics:

1. filter by room/session/system/visibility/license before or during recall;
2. retrieve vectors and keyword matches;
3. merge/rank using RRF or equivalent;
4. return evidence with citations only.

## Persistence Contract

Phase 2A adds a schema-level contract without wiring public API routes. The database has `document_sources`, `documents`, `chunks`, and `ingest_jobs` fields for source identity, license status, room scope, visibility scope, content hashes, audit linkage, and recoverable job state. The Rust side exposes `RagRepositoryTransaction` so a later SQLx implementation can claim idempotency and create source/document/chunk rows inside one transaction.

`ingest_jobs` is the recovery boundary. Jobs transition through `claimed -> parsing -> embedding -> indexed`, or terminal `pending_review` / `failed`. Terminal jobs cannot be reopened silently.

## Required Tests

- unknown license -> `pending_review`;
- commercial adapter carrying rule text -> `denied`;
- denied/pending documents do not index;
- local-only rejects cloud embedder;
- chunk hash stability;
- PL-visible retrieval cannot return `kp_only_module`;
- evidence includes citation metadata and content hash;
- ambiguous `OpenLicense` without metadata remains `pending_review`;
- ingest job state transitions reject invalid reopen paths.
