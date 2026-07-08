# BATCH-032 Test Results

## Commands

| Order | Command | Result | Notes |
|---|---|---|---|
| 1 | `cargo fmt --all -- --check` | FAIL then fixed | Rustfmt diffs were reported for new B032 files. |
| 2 | `cargo test -p trpg-platform --all-features` | FAIL then retried | Windows `link.exe` returned `LNK1104` while writing existing test executables during parallel test linking. No Rust compile error was reported. |
| 3 | `cargo fmt --all` | PASS | Formatting applied. Warning: `could not canonicalize path C:\Users\zyc14588`. |
| 4 | `cargo check -p trpg-platform --all-features` | PASS | Library and B032 module integration compiled. |
| 5 | `cargo fmt --all -- --check` | PASS | Formatting check passed. Warning: `could not canonicalize path C:\Users\zyc14588`. |
| 6 | `cargo test -p trpg-platform --test api_contracts_impl_contract_tests --all-features -- --test-threads=1` | PASS | 3 tests passed. |
| 7 | `cargo test -p trpg-platform --test deployment_ops_impl_contract_tests --all-features -- --test-threads=1` | PASS | 3 tests passed. |
| 8 | `cargo test -p trpg-platform --all-features -j 1 -- --test-threads=1` | PASS | Earlier full `trpg-platform` suite passed serially; 47 integration tests across existing and B032 modules passed before the S09 strict gate was added. |
| 9 | current-safe/previous-token/template scan over B032 changed Rust files | PASS | `rg` returned `NO_MATCH` for V3/V4/V5/V6/hash tokens, `legacy`, `serde_json::Value`, and template `Module*` names. |
| 10 | direct model-client/source-archive scan over platform source/tests | PASS | `rg` returned `NO_MATCH` for direct network/model client symbols and `source-archive` references. |
| 11 | `docker compose config` | PASS | Host Docker daemon produced a valid S09 compose config; evidence stored in `evidence/stages/S09/docker-compose-config.txt`. |
| 12 | `docker compose up -d --build --force-recreate` | PASS | Services recreated; `api` reached healthy and `reverse-proxy` started. |
| 13 | `powershell -ExecutionPolicy Bypass -File scripts\dev\smoke.ps1` | PASS | Wrote healthy API probe plus strict smoke evidence: `Result: PASS`, `docker compose ps` healthy rows, `/healthz` 200, and init wizard non-code reason. |
| 14 | `curl.exe -f http://localhost:8080/healthz` | PASS | Returned `{"status":"ok","service":"api"}`. |
| 15 | `docker compose ps` | PASS | Core S09 services reported healthy: web, api, realtime, agent-worker, postgres, redis, nats, minio, reverse-proxy, admin. |
| 16 | `cargo test -p trpg-platform --test s09_fixture_acceptance_contract_tests --all-features --target-dir target\s09-acceptance-rerun` | PASS | The tightened S09 gate parses fixture/evidence and passes after rerunning `scripts/dev/smoke.ps1`. |
| 17 | `cargo clippy -p trpg-platform --all-targets --all-features --target-dir target\s09-gate-check -- -D warnings` | PASS | No clippy warnings. |
| 18 | `cargo test -p trpg-platform --all-features --target-dir target\s09-acceptance-final -j 1 -- --test-threads=1` | PASS | Full `trpg-platform` suite passed serially with 50 integration tests, including the S09 runtime-evidence gate. A prior rerun against `target\s09-acceptance-rerun` hit a Windows `LNK1104` executable lock, not a Rust test failure. |

## B032 New Test Files

- `api_contracts_impl_contract_tests.rs`: 3 passed.
- `deployment_ops_impl_contract_tests.rs`: 3 passed.
- `observability_impl_contract_tests.rs`: 3 passed.
- `plugin_sdk_impl_contract_tests.rs`: 3 passed.
- `policy_authz_impl_contract_tests.rs`: 3 passed.
- `reliability_performance_impl_contract_tests.rs`: 3 passed.
- `security_privacy_copyrightmpl_contract_tests.rs`: 3 passed.
- `s09_fixture_acceptance_contract_tests.rs`: 3 passed.

## Stage Acceptance Boundary

S09 package-level Rust primary checks passed for `trpg-platform`. The strict S09 runtime-evidence gate now parses the detailed fixture and runtime evidence, and passes only after `docker compose config`, `docker compose up -d --build --force-recreate`, `scripts/dev/smoke.ps1`, and `/healthz` produce real PASS evidence. `init_wizard_completes` is covered by explicit non-code unavailable evidence, not by a false PASS claim.
