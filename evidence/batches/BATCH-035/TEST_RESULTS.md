# BATCH-035 Test Results

Batch: `BATCH-035-09-security-governance`
Repair date: `2026-07-09`

## Minimal Related Checks

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS; Windows path canonicalize warning only |
| `cargo check -p trpg-security-governance --all-features` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo test -p trpg-security-governance --all-features` | PASS, 13 integration tests; `visibility_enforcement_points_redacts_stage_cases` now binds S04 detailed visibility fixture, permission matrix fixture, and visibility redaction fixture |

## Stage-Oriented Checks

| Command | Result |
|---|---|
| `cargo test --workspace --all-features` | PASS |
| `cargo test -p trpg-domain-core --test visibility_leakage_tests_contract_tests --all-features` | PASS, 2 tests |
| `cargo test -p trpg-domain-core --test openfga_opa_visibility_contract_tests --all-features` | PASS, 2 tests |
| `git diff --check` | PASS with Windows line-ending warnings for `Cargo.lock` and `Cargo.toml` |

## OPA Check

| Artifact or command | Result |
|---|---|
| `policy/opa/security_governance.rego` | PRESENT; covers default deny, permission matrix allow/deny, and visibility leakage denial |
| `policy/opa/security_governance_test.rego` | PRESENT; covers default deny, keeper_only export denial, private_to_player summary denial, ai_internal export denial, and permission matrix allow/deny |
| `opa version` | PASS, OPA v1.18.2 installed at `C:\Users\zyc14588\.cargo\bin\opa.exe` |
| `opa test policy/opa` | PASS, 11/11 tests |
