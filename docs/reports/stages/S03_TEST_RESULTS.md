# S03 Test Results

Date: `2026-07-05`

## PASS

- `cargo test -p trpg-data-eventing --test event_store_contract`: 4 passed.
- `cargo test -p trpg-data-eventing --test projection_replay`: 2 passed.
- `cargo test -p trpg-data-eventing --test batch_025_data_eventing_contract_tests`: 5 passed.
- `cargo test -j 1 -p trpg-data-eventing --all-features`: B024 8 passed, B025 5 passed, event store 4 passed, projection replay 2 passed.
- `cargo fmt --all -- --check`: passed with the existing Windows canonicalize warning.
- `cargo clippy -p trpg-data-eventing --all-features -- -D warnings`: passed.
- `sqlx migrate run`: applied `20260705000100/migrate create data eventing event store`.
- `sqlx migrate revert`: applied `20260705000100/revert create data eventing event store`.
- `sqlx migrate run`: reapplied `20260705000100/migrate create data eventing event store`.

## Notes

The all-features command is recorded with `-j 1` because Windows intermittently locked freshly built test executables during parallel linking. Sequential execution passed.

Full command output is recorded in `evidence/batches/BATCH-025/TEST_RESULTS.md`.
