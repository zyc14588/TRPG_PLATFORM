# BATCH-034 Test Results

## Commands

| Command | Result | Notes |
|---|---|---|
| `cargo test -p trpg-platform --test security_privacy_copyright_contract_tests` | PASS | 8 tests passed. |
| `cargo fmt --all -- --check` | PASS | Initial run found rustfmt line wrapping; `cargo fmt --all` applied formatting, final check passed. |
| `cargo clippy -p trpg-platform --all-targets --all-features -- -D warnings` | PASS | Platform crate lint passed. |
| `cargo test -p trpg-platform --all-features` | FAIL, then rerun below | Windows MSVC linker failed with LNK1104 while linking many test executables concurrently. |
| `cargo test -p trpg-platform --all-features --jobs 1` | PASS | Final S09 crate check passed on final worktree. |
| `cargo clippy --workspace --all-targets --all-features --jobs 1 -- -D warnings` | PASS | Workspace lint passed. |
| `cargo test --workspace --all-features --jobs 1` | PASS | Final workspace test run passed on final worktree. |
| `rg -n "v3|v4|v5|v6|legacy|previous|2b129|1191|security_privacy_copyrightmpl" crates/trpg-platform/src/security_privacy_copyright.rs crates/trpg-platform/tests/security_privacy_copyright_contract_tests.rs` | PASS | No matches; `rg` exited 1 because no forbidden token was found. |

## Docker / Compose

Docker compose smoke was not rerun for BATCH-034 because this batch did not modify compose files, service binaries, or healthcheck scripts. Existing S09 stage evidence remains under:

- `evidence/stages/S09/docker-compose-config.txt`
- `evidence/stages/S09/docker-compose-smoke.txt`
- `evidence/stages/S09/health-checks.json`

This BATCH-034 evidence should not be treated as a fresh compose smoke result.
