# BATCH-024 Handoff

## Completed

- Added `trpg-data-eventing` to the workspace.
- Implemented 15 current-safe B024 primary modules with stable flat module files.
- Added shared data-eventing contract helpers for governed append, projection replay, current-safe naming, command/event schema fields, NATS subjects, metrics, and derived read-model declarations.
- Added current-safe SQL migration contract constants for Event Store, outbox, and projection checkpoint.
- Added a concrete current-safe SQLx migration entry under `migrations/`.
- Added batch-level and S03 fixture-driven contract tests covering prompt mapping, current-safe naming, authority guard, direct write rejection, expected version, idempotency, visibility replay, fact provenance, schema fields, NATS subjects, RAG metadata fixture binding, projection hash stability, and migration contract fields.
- Added docs-governance trace file for `data_eventing::m_06_data_eventing`.

## Unresolved Risks

- Live `sqlx migrate run` still requires a caller-provided `DATABASE_URL`; repeatable migration evidence is covered by `cargo test --test event_store_contract`.
- No live NATS, Redis, WebSocket, OpenAPI, or database integration was introduced. B024 currently provides contract-level boundaries and tests.
- The user-supplied primary count said 0, while `batches/B024.md` and current-safe maps contain 15 primary rows. This execution used the authoritative batch/current-safe mapping.

## Next Batch Handoff

- Later `06-data-eventing` batches can replace contract constants with concrete SQLx migrations and runtime repositories once their prompts explicitly authorize those files.
- Keep using `source-archive/**` only as provenance.
- Do not allow NATS, Redis, projection workers, RAG snapshots, frontend, business handlers, or AI agents to become canon writers.
- Preserve `Command -> Workflow -> Decision -> Event Store -> Projection` and visibility/fact provenance propagation when adding real persistence and transport integrations.
