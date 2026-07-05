# BATCH-024 Prompt Traceability

## Summary

- Declared prompt rows: 25
- User-supplied primary count: 0
- Current map/manifest primary rows used for execution: 15
- Supplemental rows: 9
- Docs-governance rows: 1
- New Rust crate: `trpg-data-eventing`
- Rust source modules changed or created: 16 including `lib.rs`
- Rust test files changed or created: 3 contract test targets
- New concrete migration files: 1
- Migration contract constants: 3 current-safe SQL statements in `persistence_migrations`
- NATS subjects: `trpg.events.appended`, `trpg.projection.rebuild.requested`
- Current-safe naming gate: enforced by `is_current_safe_name` and negative tests

## Row Results

| Prompt | Role | Evidence | Result |
|---|---|---|---|
| CODEX-0056 / P0008 | docs-governance | `docs/codex/06-data-eventing/m_06_data_eventing.md` | PASS |
| CODEX-0057 / P0001 | primary | `crates/trpg-data-eventing/src/cache_redis.rs` | PASS |
| CODEX-0058 / P0002 | primary | `crates/trpg-data-eventing/src/database_schema_index.rs` | PASS |
| CODEX-0059 / P0003 | primary | `crates/trpg-data-eventing/src/event_bus_nats.rs` | PASS |
| CODEX-0060 / P0004 | primary | `crates/trpg-data-eventing/src/event_schema_index.rs` | PASS |
| CODEX-0061 / P0005 | primary | `crates/trpg-data-eventing/src/event_store_projections.rs` | PASS |
| CODEX-0062 / P0006 | primary | `crates/trpg-data-eventing/src/outbox_projection_workers.rs` | PASS |
| CODEX-0063 / P0007 | primary | `crates/trpg-data-eventing/src/persistence_migrations.rs` | PASS |
| CODEX-0064 / P0009 | primary | `crates/trpg-data-eventing/src/snapshot_strategy.rs` | PASS |
| CODEX-0585 / P0010 | primary | `crates/trpg-data-eventing/src/adr_0002_event_sourcing_cqrs_event_sourcing_cqrs.rs` | PASS |
| CODEX-0586 / P0011 | primary | `crates/trpg-data-eventing/src/adr_0004_nats_jetstream.rs` | PASS |
| CODEX-0587 / P0012 | primary | `crates/trpg-data-eventing/src/adr_0005_postgres_pgvector_postgre_sql_pgvector.rs` | PASS |
| CODEX-0588 / P0013 | primary | `crates/trpg-data-eventing/src/adr_0010_rag_snapshot_rag_snapshot.rs` | PASS |
| CODEX-0589 / P0014 | supplemental | Merged into `cache_redis` contract and tests | PASS |
| CODEX-0590 / P0015 | supplemental | Merged into `database_schema_index` contract and tests | PASS |
| CODEX-0591 / P0016 | supplemental | Merged into `event_bus_nats` contract and tests | PASS |
| CODEX-0592 / P0017 | primary | `crates/trpg-data-eventing/src/event_json_schema_source_contract.rs` | PASS |
| CODEX-0593 / P0018 | supplemental | Merged into `event_schema_index` contract and tests | PASS |
| CODEX-0594 / P0019 | supplemental | Merged into `event_store_projections` contract and tests | PASS |
| CODEX-0595 / P0020 | primary | `crates/trpg-data-eventing/src/event_store_sqlx_outbox_projection.rs` | PASS |
| CODEX-0596 / P0023 | primary | `crates/trpg-data-eventing/src/redis_cache_presence.rs` | PASS |
| CODEX-0597 / P0026 | supplemental | Merged into `event_bus_nats` contract and tests | PASS |
| CODEX-0598 / P0025 | supplemental | Merged into `event_store_projections` contract and tests | PASS |
| CODEX-0599 / P0021 | supplemental | Merged into `persistence_migrations` contract and tests | PASS |
| CODEX-0600 / P0022 | supplemental | Merged into `cache_redis` contract and tests | PASS |

## Governance Checks

- Formal write helper requires `AuthorityContract`, `CommandEnvelope`, and `EventStore`; there is no direct AI, business, cache, projection, NATS, or provider write path.
- Event Store append preserves visibility, fact provenance, correlation id, causation id, idempotency key, and authority contract version.
- Projection replay is derived from stored events and can be filtered through `Visibility`.
- S03 fixture-driven tests bind `fixtures/stages/S03_stage_acceptance_fixture.v1.json.md`, `fixtures/stages/detailed/S03_event_store_projection_hash.current.json.md`, `test-data/event_store_stream_cases.md`, `test-data/rag_snapshot_cases.md`, and `test-data/api_ws_contract_samples.md`.
- `event_store_contract` and `projection_replay` are now concrete Cargo test targets.
- Cache, outbox, NATS, migration, schema, and RAG surfaces are modeled as rebuildable or derived contracts, not canon.
- Historical source-derived strings appear only in negative tests and deny-list checks, not as current module, event, NATS, metric, migration, workflow, or output names.
