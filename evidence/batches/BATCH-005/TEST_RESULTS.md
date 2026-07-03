# BATCH-005 Test Results

## Minimal Related Checks

- PASS: cargo test -p trpg-shared-kernel --test open_source_reference_matrix_impl_contract_tests --all-features
  - 4 passed; 0 failed.
- PASS: cargo test -p trpg-shared-kernel --test system_context_impl_contract_tests --all-features
  - 4 passed; 0 failed.
- PASS: cargo test -p trpg-shared-kernel --test technology_selection_rust_impl_contract_tests --all-features
  - 4 passed; 0 failed.

## S01 Stage Checks

- PASS: cargo fmt --all -- --check
- PASS: cargo clippy -p trpg-shared-kernel --all-targets --all-features -- -D warnings
- PASS: cargo test -p trpg-shared-kernel --all-features
  - 65 integration tests passed; 0 failed; lib/doc test harnesses had 0 tests.

## Non-blocking Tool Warning

Cargo printed `warn: could not canonicalize path C:\Users\zyc14588` during checks. The commands still exited 0 and produced passing build/test results.
