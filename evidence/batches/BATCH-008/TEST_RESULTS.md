# BATCH-008 Test Results

## Commands

| Command | Result | Notes |
|---|---|---|
| `cargo check -p trpg-domain-core --all-features` | PASS | Domain-core compiles after B008 modules. |
| `cargo fmt --all -- --check` | PASS | Final formatting check passed. |
| `cargo clippy -p trpg-domain-core --all-targets --all-features -- -D warnings` | PASS | No warnings. |
| `cargo test -p trpg-domain-core --all-features -j 1` | PASS | 37 tests passed across existing S02 tests and new B008 contract tests. |
| `cargo test -p trpg-domain-core authority --all-features -j 1` | PASS | S02 authority-focused stage check passed. |
| `cargo test -p trpg-domain-core visibility --all-features -j 1` | PASS | S02 visibility-focused stage check passed. |
| `cargo test --workspace --all-features -j 1` | PASS | Workspace test gate passed after B008 changes. |

## Notes

- An earlier parallel `cargo test -p trpg-domain-core --all-features` run hit Windows linker `LNK1104` while creating multiple test executables concurrently. The same test suite passed with Cargo `-j 1`.
- No SQLx migrations, OpenAPI snapshots, NATS/WebSocket contracts, or provider boundary tests were run because B008 did not add those artifacts.
- A red-line scan of the new B008 files found no direct model provider terms or historical source-derived output names. The only DirectAgent/DirectBusiness hits are deliberate negative-path tests and guard code.
