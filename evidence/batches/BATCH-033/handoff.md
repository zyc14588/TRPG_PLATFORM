# BATCH-033 Handoff

## Completed

- Added current-safe platform governance modules:
  - `crates/trpg-platform/src/api_contracts.rs`
  - `crates/trpg-platform/src/plugin_sdk.rs`
  - `crates/trpg-platform/src/policy_authz.rs`
- Exported the modules from `crates/trpg-platform/src/lib.rs`.
- Added focused contract tests:
  - `crates/trpg-platform/tests/api_contracts_contract_tests.rs`
  - `crates/trpg-platform/tests/plugin_sdk_contract_tests.rs`
  - `crates/trpg-platform/tests/policy_authz_contract_tests.rs`
- Wrote B033 evidence under `evidence/batches/BATCH-033/`.

## Governance Coverage

- Formal writes accept `CommandEnvelope` and append through shared `EventStore`.
- Shared kernel validation continues to enforce idempotency key, expected version, Authority mode, actor role, formal write path, visibility, fact provenance, correlation id, and causation id.
- Plugin grants reject `DirectAgent` and `DirectBusiness`.
- Policy authorization fails closed when either OpenFGA or OPA decision denies.
- Visibility and Fact Provenance survive event replay and are filtered by principal scope.

## Risks / Notes

- Default parallel `cargo test -p trpg-platform` failed on Windows linker LNK1104 while opening several test output exe files. Serial `cargo test -p trpg-platform -j 1` passed, and full workspace serial tests also passed.
- `git diff --check` passed but Git reported that `crates/trpg-platform/src/lib.rs` may be converted from LF to CRLF on next Git touch.
- This batch did not start any later batch and did not use `source-archive/**` as executable prompt input.

## Next Batch

The next batch can treat B033 current-safe modules and tests as present. If it runs platform tests on this Windows environment, prefer `-j 1` for large `cargo test` invocations unless the linker output-lock issue is resolved.
