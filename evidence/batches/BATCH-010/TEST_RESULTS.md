# BATCH-010 Test Results

Batch: BATCH-010-02-domain-core - Strict Governance Final  
Date: 2026-07-04  

## Minimal Related Checks

| Command | Result | Notes |
|---|---|---|
| `cargo fmt --all` | Pass | Formatting applied; cargo emitted a non-fatal `could not canonicalize path C:\Users\zyc14588` warning. |
| `cargo check -p trpg-domain-core --all-features` | Pass | Domain-core compiled successfully. |
| `cargo test -p trpg-domain-core --all-features --test adr_0003_authority_contract_contract_tests` | Pass | 4 passed. |
| `cargo test -p trpg-domain-core --all-features --test character_combat_san_chase_contract_tests` | Pass | 2 passed. |
| `cargo test -p trpg-domain-core --all-features --test event_sourcing_projection_contract_tests` | Pass | 2 passed. |
| `cargo test -p trpg-domain-core --all-features --test investigation_clue_npc_time_contract_tests` | Pass | 2 passed. |
| `cargo test -p trpg-domain-core --all-features --test rule_runtime_coc7_contract_tests` | Pass | 2 passed. |

## Stage Checks

| Command | Result | Notes |
|---|---|---|
| `cargo test -p trpg-domain-core --all-features` | Environment fail | First run failed during parallel MSVC linking with repeated `LNK1104` output-exe open errors; no Rust compile or test assertion failure was reported. |
| `cargo test -p trpg-domain-core --all-features --jobs 1` | Pass | Full S02 domain-core test suite passed with serial linking. |
| `cargo test -p trpg-domain-core --all-features --jobs 1 authority` | Pass | S02 authority filter passed. |
| `cargo test -p trpg-domain-core --all-features --jobs 1 visibility` | Pass | S02 visibility filter passed. |

## Quality Checks

| Command | Result | Notes |
|---|---|---|
| `cargo fmt --all -- --check` | Pass | Non-fatal canonicalize warning only. |
| `cargo clippy -p trpg-domain-core --all-targets --all-features -- -D warnings` | Pass | Domain-core clippy clean under B010 scope. |

## Coverage Notes

- New B010 current-safe tests added 12 focused assertions.
- Denied authority or direct-agent write cases append zero events.
- Keeper-only event replay remains hidden from player principals.
- Fact provenance is copied from command envelope to event envelope.
- Full S02 fixture acceptance tests passed as part of the serial full domain-core run.
