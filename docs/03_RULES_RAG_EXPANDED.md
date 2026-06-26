# Rules/RAG/Ingestion Expanded Prompt

This reference expands the active Phase 2 target: Rules/RAG/Ingestion. Do not implement realtime/WebSocket/Redis/agent UI work in this phase unless all P2 RAG acceptance tests already pass.

## Mandatory context

Read before editing:

- `AGENTS.md`
- `CODEX_MASTER_PROMPT.md`
- `DECISIONS.md`
- `docs/PRODUCT_SYSTEM_DESIGN.md`
- `docs/BACKEND_ARCHITECTURE.md`
- `docs/LEGAL_POLICY.md`
- `docs/RAG_DESIGN.md`
- `docs/P2_RAG_IMPLEMENTATION_SPEC.md`
- `docs/P2_RAG_ACCEPTANCE_TESTS.md`
- `docs/SECURITY_RLS_POLICY.md`

## Scope

Implement the first executable RAG path:

- legal source gate;
- source/document/chunk metadata;
- markdown/plain-text chunking;
- deterministic local embedding for tests and LocalOnly mode;
- in-memory vector store smoke path;
- SQLx persistence contract;
- permission-first retrieval;
- evidence with citation/provenance;
- minimal ingest/query/review APIs;
- OpenAPI and frontend DTO updates.

## Hard boundaries

- Do not include commercial copyrighted rule text.
- Do not index or retrieve pending/denied license content.
- Do not call cloud providers from LocalOnly rooms.
- Do not return KP-only/module/private chunks to PL/observer/public screen.
- Do not let LLM output mutate game state.
- Do not duplicate license enums between `rag_core` and `document_ingestor`.

## Stop points

### P2-A: `rag_core` local kernel

- Implement RAG domain types.
- Implement license decision.
- Implement `MarkdownChunker`.
- Implement deterministic local embedder.
- Implement in-memory vector store / hybrid retrieval sufficient for tests.
- Move document_ingestor license status to rag_core.

Run:

```bash
cargo test -p rag_core -p document_ingestor
```

### P2-B: storage repository and RLS

- Add additive migration for RAG tables/policies if needed.
- Implement repository transaction traits.
- Enforce `license_status='allowed'` on normal retrieval.
- Add RLS tests for PL/KP/license boundaries.

Run:

```bash
cargo test -p storage --all-features
cargo sqlx migrate run
cargo sqlx prepare --check --workspace
```

### P2-C: server API

- Add ingest/query/review routes.
- Add request validation and body limits.
- Add OpenAPI paths and route-contract tests.
- Do not return raw DB entities.

Run:

```bash
cargo test -p server --all-features
```

### P2-D: frontend minimal workflow

- Add minimal KP document ingest/review/query screens.
- Add strict DTO parsers.
- Show citations and license status.

Run:

```bash
pnpm lint
pnpm typecheck
pnpm test
pnpm test:e2e
pnpm build
```

## Required tests

Implement all acceptance tests listed in `docs/P2_RAG_ACCEPTANCE_TESTS.md`.

## Completion report

At the end, update `docs/status/PHASE_2_REPORT.md` with:

- changed files;
- design decisions;
- exact commands run and results;
- residual risks;
- out-of-scope items;
- confirmation that no unauthorized commercial rule text was added.
