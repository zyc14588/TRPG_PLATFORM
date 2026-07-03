# BATCH-006 Test Results

## Target Primary Checks

- PASS: cargo test -p trpg-shared-kernel --test adr_0001_rust_first_contract_tests --test technology_selection_rust_contract_tests --all-features
  - 8 target contract tests passed; 0 failed.

## S01 Stage Checks

- PASS: cargo fmt --all -- --check
- PASS: cargo clippy -p trpg-shared-kernel --all-targets --all-features -- -D warnings
- PASS: cargo clippy --workspace --all-targets --all-features -- -D warnings
- PASS: cargo test -p trpg-shared-kernel --all-features
  - The first parallel-link attempt hit Windows `LNK1104` file-lock errors on existing test executables; rerun with `CARGO_BUILD_JOBS=1` exited 0.
- PASS: cargo test --workspace --all-features
  - Run with `CARGO_BUILD_JOBS=1` to avoid the same Windows linker file-lock race; exited 0.
- PASS: cargo check -p trpg-shared-kernel

## Fixture Coverage

- PASS: S01 detailed fixture `fixtures/stages/detailed/S01_foundation_shared_kernel.current.json.md` is included by `crates/trpg-shared-kernel/tests/shared_kernel_contract_tests.rs`.
- PASS: fixture expectations for `UNKNOWN_VISIBILITY_LABEL`, `INVALID_ENTITY_ID`, closed visibility labels, `private_to_player` redaction, and `ai_internal` non-player visibility are covered by `shared_kernel_contract_tests.rs`.

## Static Boundary Checks

- PASS: shared-kernel direct LLM/provider smoke
  - Command: rg -n "openai|ollama|llama|chat_completion|responses" crates/trpg-shared-kernel
  - Result: NO direct call path. `ModelProvider` only appears as an enum value in open-source reference classification.
- PASS: supplemental prompt IDs do not appear in `crates/trpg-shared-kernel/src` or `crates/trpg-shared-kernel/tests`.
- PASS: git diff --check
  - No whitespace or patch formatting errors in tracked-file diff. Git printed LF/CRLF working-copy warnings for Rust files.

## Non-applicable Checks

- pnpm: Not applicable. No `package.json`, `pnpm-lock.yaml`, or `pnpm-workspace.yaml` exists in this repository.
- docker compose: Not applicable for B006/S01 shared-kernel repair. No compose file exists in this repository.

## Non-blocking Tool Warning

Cargo printed `warn: could not canonicalize path C:\Users\zyc14588` during fmt, clippy, and tests. The commands still exited 0.
