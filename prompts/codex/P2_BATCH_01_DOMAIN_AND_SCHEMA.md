# Codex Batch 01 — RAG Domain and Schema Contracts

Start only after Batch 00 is green.

## Read first

- `docs/p2/01_P2_MASTER_SPEC.md`
- `docs/p2/02_RAG_CORE_DOMAIN_SPEC.md`
- `docs/p2/06_SECURITY_LEGAL_PROVIDER_POLICY.md`
- existing `docs/P2_RAG_IMPLEMENTATION_SPEC.md`
- existing `docs/P2_RAG_ACCEPTANCE_TESTS.md`

## Tasks

1. Normalize `crates/rag_core` domain types for source, license, visibility, document, chunk, citation, evidence, retrieval query/result.
2. Add/normalize traits: `Chunker`, `Embedder`, `VectorStore`, `KeywordIndex` if needed, `HybridRetriever`.
3. Implement deterministic local pieces for tests: Markdown chunker, deterministic embedder, in-memory vector store.
4. Ensure `document_ingestor` reuses `rag_core` license/status/visibility types instead of redefining them.
5. Add unit tests for license decisions, normalization, chunk hashes, heading paths, local embedder determinism, and top_k bounds.
6. Add schema/OpenAPI DTO stubs only where they clarify later server work; avoid implementing routes in this batch.

## Constraints

- No DB migrations in this batch unless needed to keep compile green.
- No frontend changes except generated schema/types if the repo already has that workflow.
- No real cloud provider calls.

## Checks

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p rag_core -p document_ingestor
```

## Completion response

List domain types added/changed, tests run, and any schema decisions.
