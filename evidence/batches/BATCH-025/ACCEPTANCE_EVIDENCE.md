# BATCH-025 Acceptance Evidence

Batch: `BATCH-025-06-data-eventing`
Mode: Strict Governance Final

## Implemented Scope

- Added current-safe primary modules for the 11 B025 implementation prompts:
  - `persistence_postgresql`
  - `redis_presence`
  - `nats_jet_stream`
  - `postgre_sql_sq_lx_pgvector`
  - `sqlx_migrations`
  - `event_sourcing_snapshot_projection`
  - `schema`
  - `readme`
  - `snapshot`
  - `event_command_json_schema`
  - `sqlx_migrations_contract`
- Registered B025 contracts through `batch_025_data_event_contracts()` and extended `all_data_event_contracts()`.
- Added B025 named command/event/service/repository/error artifacts through the existing data-eventing contract pattern.
- Kept all formal writes on `CommandEnvelope -> AuthorityContract -> EventStore -> Projection`.
- Did not add direct OpenAI, Ollama, llama.cpp, local model, business-layer LLM, direct agent DB write, or direct business formal-state write paths.
- Did not use `source-archive/**` as executable prompt material.

## Supplemental Prompt Handling

The 14 B025 supplemental prompts were treated as constraints only. They did not create Rust implementation outputs. Their NATS, Redis, projection, migration, schema, README, and snapshot constraints are covered by the owning primary modules or existing B024 owners.

## Tests Added Or Updated

- Added `crates/trpg-data-eventing/tests/batch_025_data_eventing_contract_tests.rs`.
- Updated the B024 aggregate test so it still verifies all B024 contracts while allowing later batch contracts to be appended to the global registry.
- Updated `crates/trpg-data-eventing/tests/projection_replay.rs` so S03 detailed fixture expected events, expected records, expected errors, and failure cases are executable assertions.
- Converted the data-eventing migration into reversible SQLx `.up.sql` / `.down.sql` files and live-verified run/revert/run against disposable PostgreSQL.

## Acceptance Notes

- Current-safe names are asserted for module, event, schema, NATS subject, metric, migration, and required column identifiers.
- `expected_version`, idempotency, visibility, fact provenance, authority contract validation, and direct-agent bypass denial are covered by tests.
- Projection, snapshot, and pgvector/RAG outputs remain rebuildable read models derived from Event Store evidence.
- S03 projection replay hash is asserted as `sha256:a83861bce178f274e6a2e809c790770577445268b48fedfb889af4b87f8c1c50`.
- Required S03 evidence exists under `docs/reports/stages/` and `evidence/stages/S03/`.

## SQLx Live Migration

Closed. `sqlx migrate run`, `sqlx migrate revert`, and `sqlx migrate run` were executed against a disposable PostgreSQL container. Full output is in `evidence/batches/BATCH-025/TEST_RESULTS.md`.
