# BATCH-028 Test Results

Batch: `BATCH-028-06-data-eventing`
Stage: `S03`

## Commands

| Order | Command | Result | Notes |
|---:|---|---|---|
| 1 | `cargo fmt --all` | PASS | Completed with warning: `could not canonicalize path C:\Users\zyc14588`. |
| 2 | `cargo test -p trpg-data-eventing --test event_json_schema_contract_tests --all-features` | PASS | 4 tests passed. |
| 3 | `cargo test -p trpg-data-eventing --all-features` | PASS | 34 tests passed across data-eventing integration/contract tests plus doc tests. |
| 4 | `cargo fmt --all -- --check` | PASS | Completed with warning: `could not canonicalize path C:\Users\zyc14588`. |
| 5 | `cargo test -p trpg-data-eventing --test event_store_contract --all-features` | PASS | 4 tests passed. |
| 6 | `cargo test -p trpg-data-eventing --test projection_replay --all-features` | PASS | 2 tests passed. |
| 7 | `cargo test -p trpg-data-eventing --test event_json_schema_contract_tests --all-features` | FAIL then PASS on rerun | First rerun failed at Windows link step with `LNK1104` opening the test exe, consistent with a transient file lock. Immediate standalone rerun passed 4 tests. |
| 8 | `cargo test -p trpg-data-eventing --all-features` | PASS | Final stage crate check passed. |

## Not Run

- `sqlx migrate run`: not run because B028/P0107 did not create or modify migrations.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: not run; batch scope only required minimal related and S03 crate tests.
- Full `cargo test --workspace --all-features`: not run; no workspace-wide shared API beyond `trpg-data-eventing` registration changed.

## Contract Coverage

- `event_json_schema_contract_maps_to_current_safe_primary_output`
- `event_json_schema_catalog_declares_governed_command_and_event_fields`
- `event_json_schema_appends_only_through_governed_event_store_path`
- `event_json_schema_preserves_visibility_provenance_and_fixture_bindings`

