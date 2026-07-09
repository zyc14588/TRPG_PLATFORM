# BATCH-040 Test Results

Batch: `BATCH-040-10-testing-quality`
Stage: `S11-testing-quality-golden-ci`

## Commands

| Order | Command | Result | Notes |
| ---: | --- | --- | --- |
| 1 | `cargo test -p trpg-testing --all-features` | PASS after repair | First run failed because `decision_trace_map` still expected 21 primary contracts; updated B040 trace count and expected 23. |
| 2 | `cargo test -p trpg-testing --test golden_scenarios_ci --all-features` | PASS | S11 golden scenario stage gate. |
| 3 | `cargo test -p trpg-testing --test visibility_leakage --all-features` | PASS | S11 visibility leakage stage gate. |
| 4 | `cargo test -p trpg-testing --test model_certification_tests --all-features` | PASS | S11 model certification stage gate. |
| 5 | `cargo fmt --all -- --check` | PASS after `cargo fmt --all` | Initial check requested formatting in `lib.rs`; formatter applied mechanically. |
| 6 | `cargo test -p trpg-testing --all-features` | PASS | Re-run after formatting. |
| 7 | `cargo test -p trpg-testing --test golden_scenarios_ci --all-features` | PASS | Re-run after formatting. |
| 8 | `cargo test -p trpg-testing --test visibility_leakage --all-features` | PASS | Re-run after formatting. |
| 9 | `cargo test -p trpg-testing --test model_certification_tests --all-features` | PASS | Re-run after formatting. |
| 10 | `git diff --check` | PASS | Only CRLF normalization warnings were printed. |

## Observed Warnings

- Cargo printed `warn: could not canonicalize path C:\Users\zyc14588`. The warning did not fail any command.
- Git printed CRLF normalization warnings during `git diff --check`; no whitespace errors were reported.

## Not Run

- `cargo clippy --workspace --all-targets --all-features -- -D warnings` was not run for this batch. The user-required sequence asked for minimal related and S11 stage checks; no code path outside `trpg-testing` was changed.
- `cargo test --workspace --all-features` was not run; B040 scope is limited to S11 `trpg-testing` changes.
