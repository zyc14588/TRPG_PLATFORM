# S11 Test Results

Stage: `S11-testing-quality-golden-ci`
Batch evidence repaired: `BATCH-041-10-testing-quality`
Run date: 2026-07-09

## Commands

| Order | Command | Result | Evidence notes |
| ---: | --- | --- | --- |
| 1 | `cargo fmt --all -- --check` | PASS | Formatting check passed; canonicalize warning only. |
| 2 | `cargo test -p trpg-testing --test golden_scenarios_ci_contract_tests --all-features` | PASS | 3 passed, 0 failed. Covers B041 primary, expected records/errors, direct agent and authority bypass. |
| 3 | `cargo test -p trpg-testing --test golden_scenarios_ci --all-features` | PASS | 1 passed, 0 failed. S11 golden scenario gate. |
| 4 | `cargo test -p trpg-testing --test visibility_leakage --all-features` | PASS | 1 passed, 0 failed. S11 visibility leakage gate. |
| 5 | `cargo test -p trpg-testing --test model_certification_tests --all-features` | PASS | 1 passed, 0 failed. S11 model certification gate. |
| 6 | `cargo test -p trpg-testing --all-features` | PASS | Full `trpg-testing` package suite passed; 24 primary contract rows covered. |
| 7 | `cargo test --workspace --all-features` | PASS | Workspace tests and doc-tests passed. |
| 8 | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS | Workspace clippy passed. |
| 9 | `git diff --check` | PASS | CRLF normalization warnings only; no whitespace errors. |

## Fixture Assertions Covered

- `ScenarioTestReport`: `steps`, `dice`, `decisions`, `final_state_hash`.
- `ExportDiffReport`: `player_export_hash`, `keeper_export_hash`, `audit_export_hash`, `redacted_fields`.
- `VISIBILITY_LEAKAGE_DETECTED`
- `GOLDEN_SCENARIO_RULE_VIOLATION`
- `KEEPER_SECRET_REVEALED`

## Stage Gate Mapping

| S11 gate | Command/evidence |
| --- | --- |
| Golden Scenario fixed input/replay | `cargo test -p trpg-testing --test golden_scenarios_ci --all-features`; `evidence/stages/S11/golden-scenario.txt` |
| Visibility/private export leakage | `cargo test -p trpg-testing --test visibility_leakage --all-features`; `evidence/stages/S11/visibility-leakage.txt`; `evidence/stages/S11/export-diff.txt` |
| Model certification Level 4 | `cargo test -p trpg-testing --test model_certification_tests --all-features` |
| Contract / replay / trace map | `cargo test -p trpg-testing --all-features`; `docs/reports/stages/S11_TRACEABILITY.md` |
| Workspace regression | `cargo test --workspace --all-features`; `cargo clippy --workspace --all-targets --all-features -- -D warnings` |

## Non-applicable Checks

- pnpm: not applicable; no frontend package manifest is present.
- Docker: not applicable to B041 evidence repair; `docker-compose.ci.yml` exists but deployment smoke is S09/S13-owned.
- SQLx/OpenAPI/NATS/WebSocket generation: not applicable; this repair changed only evidence documents.

## Warnings

- Cargo warning: `warn: could not canonicalize path C:\Users\zyc14588`; non-fatal.
- Git warning: CRLF normalization warnings; non-fatal.
