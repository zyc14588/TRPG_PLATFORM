# BATCH-024-06-data-eventing Strict Governance Final Plan

Baseline date: 2026-07-05

## Scope Guard

- Batch file: `batches/B024.md`
- Stage: `stages/s03-data-eventing-persistence`
- Declared prompt rows: 25
- User-supplied primary count: 0
- Current map/manifest primary implementation rows: 15
- Supplemental rows: 9
- Docs-governance rows: 1
- Current-safe target crate: `trpg-data-eventing`
- `source-archive/**` is provenance only. Historical version names, source paths, and hash fragments must not become current modules, migrations, events, metrics, NATS subjects, workflows, tests, or outputs.
- Supplemental rows are applied only as constraints merged into their primary prompt owner. They do not create Rust src/test ownership.

## Prompt Mapping

| Prompt | Role | Current-safe target | Allowed change | Test responsibility |
|---|---|---|---|---|
| CODEX-0056 / P0008 | docs-governance | `docs/codex/06-data-eventing/m_06_data_eventing.md` | Markdown traceability only | Prompt traceability check |
| CODEX-0057 / P0001 | primary | `crates/trpg-data-eventing/src/cache_redis.rs` | Rust flat module under `data_eventing::cache_redis` | `batch_024_data_eventing_contract_tests` |
| CODEX-0058 / P0002 | primary | `crates/trpg-data-eventing/src/database_schema_index.rs` | Rust flat module under `data_eventing::database_schema_index` | `batch_024_data_eventing_contract_tests` |
| CODEX-0059 / P0003 | primary | `crates/trpg-data-eventing/src/event_bus_nats.rs` | Rust flat module under `data_eventing::event_bus_nats` | `batch_024_data_eventing_contract_tests` |
| CODEX-0060 / P0004 | primary | `crates/trpg-data-eventing/src/event_schema_index.rs` | Rust flat module under `data_eventing::event_schema_index` | `batch_024_data_eventing_contract_tests` |
| CODEX-0061 / P0005 | primary | `crates/trpg-data-eventing/src/event_store_projections.rs` | Rust flat module under `data_eventing::event_store_projections` | `batch_024_data_eventing_contract_tests` |
| CODEX-0062 / P0006 | primary | `crates/trpg-data-eventing/src/outbox_projection_workers.rs` | Rust flat module under `data_eventing::outbox_projection_workers` | `batch_024_data_eventing_contract_tests` |
| CODEX-0063 / P0007 | primary | `crates/trpg-data-eventing/src/persistence_migrations.rs` | Rust flat module plus current-safe SQL migration contract constants | `batch_024_data_eventing_contract_tests` |
| CODEX-0064 / P0009 | primary | `crates/trpg-data-eventing/src/snapshot_strategy.rs` | Rust flat module under `data_eventing::snapshot_strategy` | `batch_024_data_eventing_contract_tests` |
| CODEX-0585 / P0010 | primary | `crates/trpg-data-eventing/src/adr_0002_event_sourcing_cqrs_event_sourcing_cqrs.rs` | Rust flat module under current-safe ADR module | `batch_024_data_eventing_contract_tests` |
| CODEX-0586 / P0011 | primary | `crates/trpg-data-eventing/src/adr_0004_nats_jetstream.rs` | Rust flat module under current-safe ADR module | `batch_024_data_eventing_contract_tests` |
| CODEX-0587 / P0012 | primary | `crates/trpg-data-eventing/src/adr_0005_postgres_pgvector_postgre_sql_pgvector.rs` | Rust flat module under current-safe ADR module | `batch_024_data_eventing_contract_tests` |
| CODEX-0588 / P0013 | primary | `crates/trpg-data-eventing/src/adr_0010_rag_snapshot_rag_snapshot.rs` | Rust flat module under current-safe ADR module | `batch_024_data_eventing_contract_tests` |
| CODEX-0589 / P0014 | supplemental | Merge into `data_eventing::cache_redis` | Constraint merge only; no Rust ownership | Covered by cache and naming tests |
| CODEX-0590 / P0015 | supplemental | Merge into `data_eventing::database_schema_index` | Constraint merge only; no Rust ownership | Covered by schema tests |
| CODEX-0591 / P0016 | supplemental | Merge into `data_eventing::event_bus_nats` | Constraint merge only; no Rust ownership | Covered by outbox/NATS tests |
| CODEX-0592 / P0017 | primary | `crates/trpg-data-eventing/src/event_json_schema_source_contract.rs` | Rust flat module under `data_eventing::event_json_schema_source_contract` | `batch_024_data_eventing_contract_tests` |
| CODEX-0593 / P0018 | supplemental | Merge into `data_eventing::event_schema_index` | Constraint merge only; no Rust ownership | Covered by schema tests |
| CODEX-0594 / P0019 | supplemental | Merge into `data_eventing::event_store_projections` | Constraint merge only; no Rust ownership | Covered by replay/projection tests |
| CODEX-0595 / P0020 | primary | `crates/trpg-data-eventing/src/event_store_sqlx_outbox_projection.rs` | Rust flat module under `data_eventing::event_store_sqlx_outbox_projection` | `batch_024_data_eventing_contract_tests` |
| CODEX-0596 / P0023 | primary | `crates/trpg-data-eventing/src/redis_cache_presence.rs` | Rust flat module under `data_eventing::redis_cache_presence` | `batch_024_data_eventing_contract_tests` |
| CODEX-0597 / P0026 | supplemental | Merge into `data_eventing::event_bus_nats` | Constraint merge only; no Rust ownership | Covered by outbox/NATS tests |
| CODEX-0598 / P0025 | supplemental | Merge into `data_eventing::event_store_projections` | Constraint merge only; no Rust ownership | Covered by replay/projection tests |
| CODEX-0599 / P0021 | supplemental | Merge into `data_eventing::persistence_migrations` | Constraint merge only; no Rust ownership | Covered by migration contract tests |
| CODEX-0600 / P0022 | supplemental | Merge into `data_eventing::cache_redis` | Constraint merge only; no Rust ownership | Covered by cache and naming tests |

## Test Plan

Minimal related check:

- `cargo test -p trpg-data-eventing --test batch_024_data_eventing_contract_tests -- --nocapture`

Stage checks:

- `cargo test -p trpg-data-eventing --all-features`
- `cargo fmt --all -- --check`
- `cargo check -p trpg-data-eventing`
- `cargo clippy -p trpg-data-eventing --all-targets --all-features -- -D warnings`

Not run:

- `sqlx migrate run`, because B024 added a current-safe migration contract in Rust but did not add a runtime database URL, SQLx dependency, or concrete workspace migration directory.
- `cargo test --test event_store_contract` and `cargo test --test projection_replay`, because those exact test targets do not exist in this workspace yet. Their assertions are covered in `batch_024_data_eventing_contract_tests`.
