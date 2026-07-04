# BATCH-009 Test Results

## Commands

| Command | Purpose | Result |
|---|---|---|
| `cargo fmt --all` | Format changed Rust files before tests. | PASS |
| `cargo test -p trpg-domain-core --all-features` | Minimal related check and S02 full domain-core test pass. | PASS |
| `cargo test -p trpg-domain-core authority --all-features` | S02 authority filtered check. | PASS |
| `cargo test -p trpg-domain-core visibility --all-features` | S02 visibility filtered check. | PASS |
| `cargo fmt --all -- --check` | Final formatting verification. | PASS |

## Notes

- The full domain-core test run included the 8 new BATCH-009 contract test files and 16 new tests.
- The filtered checks also exercised the new authority and visibility/provenance replay assertions.
- `cargo test -p trpg-domain-core authority --all-features`, `cargo test -p trpg-domain-core visibility --all-features`, and `cargo fmt --all -- --check` emitted `warn: could not canonicalize path C:\Users\zyc14588`; the commands exited successfully.
