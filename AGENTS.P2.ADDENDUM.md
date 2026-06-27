# AGENTS P2 Addendum

This addendum is intended to be copied into the repository root or linked from `AGENTS.md`.

## P2 agent rules

1. Treat P2 as a security-sensitive RAG system, not a demo search box.
2. License and visibility checks happen before chunking, embedding, indexing, scoring, reranking, and API serialization.
3. PostgreSQL RLS is a required security boundary. Application-layer checks are necessary but not sufficient.
4. All external providers must sit behind traits/adapters. Tests use deterministic local providers only.
5. Every new API must be represented in `schemas/openapi.json` and covered by route-contract tests.
6. Every migration must be additive. Do not rewrite historical migrations after they have shipped.
7. Keep `crates/server/src/lib.rs` modular. New P2 handlers, DTOs, and route tests should live in modules where possible.
8. New frontend state must not trust hidden fields; test that KP-only fields are absent, not merely hidden.
9. Each batch must produce a concise status note and exact commands run.

## Disallowed shortcuts

- “Filter in UI only.”
- “Trust the client role field.”
- “Index first, filter after scoring.”
- “Use real OpenAI/Ollama in unit tests.”
- “Mark unknown license as allowed for convenience.”
- “Use `unwrap()` in request paths without a clear invariant and test.”
