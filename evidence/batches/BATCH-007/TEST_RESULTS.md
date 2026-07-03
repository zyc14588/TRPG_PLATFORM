# BATCH-007 Test Results

## Commands

| Command | Result |
|---|---|
| `cargo check -p trpg-domain-core` | PASS |
| `cargo fmt --all` | PASS |
| `cargo fmt --all -- --check` | PASS |
| `cargo test -p trpg-domain-core s02 --all-features` | PASS |
| `cargo test -p trpg-domain-core --all-features` | PASS |
| `cargo test -p trpg-domain-core authority --all-features` | PASS |
| `cargo test -p trpg-domain-core visibility --all-features` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS after replacing a too-many-arguments constructor with `DecisionRecordDraft` |
| `cargo test --workspace --all-features` | PASS |
| `git diff --check` | PASS, with LF-to-CRLF warnings for tracked Cargo files only |
| `rg -n "openai|ollama|llama|chat_completion|responses" crates/trpg-domain-core` | PASS, no matches |
| `rg -n "V3|V4|V5|V6|v3|v4|v5|v6" crates/trpg-domain-core` | PASS, no matches |

## Notes

- Windows emitted `warn: could not canonicalize path C:\Users\zyc14588` on
  Cargo commands; all commands exited successfully.
- The first clippy run failed on `decision_record_model.rs` for
  `clippy::too_many_arguments`; the implementation was changed to use
  `DecisionRecordDraft`, then clippy passed.

## Coverage

- Authority Contract immutable and fork-only.
- HUMAN_KP / AI_KP formal command mismatch rejection.
- Idempotency key duplicate and expected_version conflict.
- DecisionRecord required audit fields.
- Confirmed fact source allowlist.
- Policy default deny and visibility-aware allow.
- Fork lineage default copy vs explicit-permission copy scopes.
- Visibility most-restrictive merge and redaction outcomes.
- Event Store append and projection rebuild.
- Direct Agent formal state write rejection.
- S02 fixture-only acceptance loading for `case_id`, `expected`, `error`,
  `event`, and visibility leak fixture mappings.
