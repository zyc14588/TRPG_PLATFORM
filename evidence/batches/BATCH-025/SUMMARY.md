# BATCH-025 Summary

## Changed Files

- `crates/trpg-data-eventing/src/lib.rs`
- `crates/trpg-data-eventing/src/event_command_json_schema.rs`
- `crates/trpg-data-eventing/src/event_sourcing_snapshot_projection.rs`
- `crates/trpg-data-eventing/src/nats_jet_stream.rs`
- `crates/trpg-data-eventing/src/persistence_postgresql.rs`
- `crates/trpg-data-eventing/src/postgre_sql_sq_lx_pgvector.rs`
- `crates/trpg-data-eventing/src/readme.rs`
- `crates/trpg-data-eventing/src/redis_presence.rs`
- `crates/trpg-data-eventing/src/schema.rs`
- `crates/trpg-data-eventing/src/snapshot.rs`
- `crates/trpg-data-eventing/src/sqlx_migrations.rs`
- `crates/trpg-data-eventing/src/sqlx_migrations_contract.rs`
- `crates/trpg-data-eventing/tests/batch_024_data_eventing_contract_tests.rs`
- `crates/trpg-data-eventing/tests/batch_025_data_eventing_contract_tests.rs`
- `crates/trpg-data-eventing/tests/event_store_contract.rs`
- `crates/trpg-data-eventing/tests/projection_replay.rs`
- `fixtures/stages/detailed/S03_event_store_projection_hash.current.json.md`
- `migrations/20260705000100_create_data_eventing_event_store.up.sql`
- `migrations/20260705000100_create_data_eventing_event_store.down.sql`
- `docs/reports/stages/S03_ACCEPTANCE_EVIDENCE.md`
- `docs/reports/stages/S03_TEST_RESULTS.md`
- `docs/reports/stages/S03_TRACEABILITY.md`
- `evidence/stages/S03/event-store-contract.txt`
- `evidence/stages/S03/projection-replay-hash.txt`
- `evidence/batches/BATCH-025/WORK_PLAN.md`
- `evidence/batches/BATCH-025/TEST_RESULTS.md`
- `evidence/batches/BATCH-025/ACCEPTANCE_EVIDENCE.md`
- `evidence/batches/BATCH-025/SUMMARY.md`

## Verification

- `cargo fmt --all`
- `cargo test -p trpg-data-eventing --test batch_025_data_eventing_contract_tests`
- `cargo test -p trpg-data-eventing --all-features`
- `cargo test -p trpg-data-eventing --test event_store_contract`
- `cargo test -p trpg-data-eventing --test projection_replay`
- `cargo fmt --all -- --check`
- `cargo clippy -p trpg-data-eventing --all-features -- -D warnings`
- `sqlx migrate run`
- `sqlx migrate revert`
- `sqlx migrate run`

## Handoff

Next batch can build on `batch_025_data_event_contracts()` and the global `all_data_event_contracts()` registry. B025 SQLx migration has live run/revert/run evidence against disposable PostgreSQL.
