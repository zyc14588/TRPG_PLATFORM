# BATCH-020 Test Results

## Minimal Checks

| Command | Result | Notes |
| --- | --- | --- |
| `cargo test -p trpg-agent-runtime --test adr_0010_rag_snapshot_contract_tests` | PASS | 3 tests passed. |
| `cargo test -p trpg-agent-runtime --test evaluation_golden_scenario_contract_tests` | PASS | 3 tests passed. One immediate retry was needed after transient Windows `LNK1104` file locking on the test exe. |

## Stage Checks

| Command | Result | Notes |
| --- | --- | --- |
| `cargo fmt --all -- --check` | PASS | Cargo emitted the existing path canonicalization warning for `C:\Users\zyc14588`; formatting check succeeded. |
| `cargo test -p trpg-agent-runtime --all-features` | PASS | All `trpg-agent-runtime` tests passed, including the new B020 tests. |
| `rg -n "curl|reqwest|ureq|chat_completion|responses\(" crates\trpg-agent-runtime\src` | PASS | No matches; `rg` exit code 1 is expected for an empty search result. |

## Workspace Checks

| Command | Result | Notes |
| --- | --- | --- |
| `cargo test --workspace --all-features` | PASS | Full workspace test suite passed. |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS | Clippy completed without warnings; Cargo emitted the existing path canonicalization warning. |
| `git diff --check` | PASS | No whitespace errors; Git reported CRLF normalization warnings for touched tracked Rust files. |
