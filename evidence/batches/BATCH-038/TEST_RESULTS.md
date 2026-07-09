# BATCH-038 Test Results

Batch: `BATCH-038-10-testing-quality`  
Stage: `S11 testing quality golden CI`  
Result: PASS

## Minimal Relevant Checks

| Command | Result | Notes |
| --- | --- | --- |
| `cargo check -p trpg-testing --all-features` | PASS | New crate compiles against `trpg-shared-kernel` and `trpg-agent-runtime`. |
| `cargo fmt --all -- --check` | PASS | First run reported formatting diffs in new files; `cargo fmt --all` was applied and the final check passed. |
| `cargo test -p trpg-testing --all-features` | PASS | 18 integration tests passed; crate unit/doc suites had 0 runnable tests. |
| `git diff --check` | PASS | No whitespace or conflict-marker errors. Cargo emitted non-failing CRLF warnings for `Cargo.lock` and `Cargo.toml`. |

## S11 Stage Checks

The S11 test plan names stage test targets. Because this repository root is a virtual workspace, the commands were run with `-p trpg-testing` to target the current batch crate.

| Command | Result | Notes |
| --- | --- | --- |
| `cargo test -p trpg-testing --test golden_scenarios_ci` | PASS | 1/1 stage gate passed; verifies golden fixture markers, formal decision path, and server dice requirement. |
| `cargo test -p trpg-testing --test visibility_leakage` | PASS | 1/1 stage gate passed; verifies restricted export token redaction. |
| `cargo test -p trpg-testing --test model_certification_tests` | PASS | 1/1 stage gate passed; verifies Level 4 requirement and silent fallback denial. |

## Non-Failing Warnings

- Cargo emitted `could not canonicalize path C:\Users\zyc14588` in some runs. Existing batches have recorded the same non-failing local path warning.
- Parallel stage checks briefly waited on the Cargo build directory file lock; all commands completed successfully.
