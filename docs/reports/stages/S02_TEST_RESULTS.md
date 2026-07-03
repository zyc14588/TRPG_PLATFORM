# S02 Test Results

## Commands

| Command | Result |
|---|---|
| `cargo fmt --all` | PASS |
| `cargo fmt --all -- --check` | PASS |
| `cargo check -p trpg-domain-core` | PASS |
| `cargo test -p trpg-domain-core s02 --all-features` | PASS |
| `cargo test -p trpg-domain-core --all-features` | PASS |
| `cargo test -p trpg-domain-core authority --all-features` | PASS |
| `cargo test -p trpg-domain-core visibility --all-features` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo test --workspace --all-features` | PASS |
| `rg --files -g package.json -g pnpm-lock.yaml -g pnpm-workspace.yaml -g docker-compose.yml -g docker-compose.yaml -g Dockerfile` | No matches; pnpm/docker not applicable for S02 |

## Fixture Tests

The S02 fixture-only repair adds `s02_fixture_acceptance_contract_tests.rs`
with four executable tests for stage policy, detailed expected records,
authority/fork fixture cases, and visibility redaction fixture cases.

The first `cargo test -p trpg-domain-core s02 --all-features` attempt hit a
transient Windows linker lock (`LNK1104`) while replacing the test binary. The
immediate rerun passed.
