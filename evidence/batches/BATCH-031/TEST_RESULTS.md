# BATCH-031 Test Results

## Commands

| Order | Command | Result | Notes |
|---|---|---|---|
| 1 | `cargo fmt --all` | PASS | Rust formatting applied. Warning: `could not canonicalize path C:\Users\zyc14588`. |
| 2 | `cargo test -p trpg-platform --all-features` | FAIL then retried | Default `target` output hit Windows linker `LNK1104` on existing test executables. |
| 3 | `CARGO_TARGET_DIR=target\\b031 cargo test -p trpg-platform --all-features -j1` | PASS | 24 B031 integration tests passed. |
| 4 | `CARGO_TARGET_DIR=target\\b031 cargo test --workspace --all-features -j1` | PASS | First 240s run timed out while still progressing; rerun with longer timeout completed successfully. |
| 5 | `cargo fmt --all -- --check` | PASS | Formatting check passed. Warning: `could not canonicalize path C:\Users\zyc14588`. |
| 6 | direct model-client scan over B031 changed sources and evidence | PASS | `NO_MATCH`. |
| 7 | previous/version-token/hash-name scan over B031 changed sources and evidence | PASS | `NO_MATCH`. |
| 8 | `git diff --check` | PASS | Only Git CRLF warnings for `Cargo.toml` and `Cargo.lock`. |

## B031 Test Files

- `background_workers_contract_tests.rs`
- `deployment_ops_contract_tests.rs`
- `local_dev_environment_contract_tests.rs`
- `object_storage_contract_tests.rs`
- `observability_contract_tests.rs`
- `performance_budget_contract_tests.rs`
- `deployment_observability_contract_tests.rs`
- `reliability_performance_contract_tests.rs`
- `observability_audit_trace_contract_tests.rs`
- `readme_contract_tests.rs`

## Stage Acceptance Boundary

Full S09 Docker Compose and `/healthz` runtime smoke checks were not run because this repository currently has no runnable compose YAML in the implementation tree; only `ci-cd/workflows-extractable/target-docker-compose-smoke.yml.md` exists. B031 was limited to the normalized current-safe Rust crate/module outputs.
