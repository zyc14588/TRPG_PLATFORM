# BATCH-027 Test Results

Batch: BATCH-027-06-data-eventing - Strict Governance Final
Date: 2026-07-07

## Minimal B027 Checks

1. B027 traceability file existence self-check
   - Command: PowerShell `Test-Path` over the 10 documentation-or-traceability output paths.
   - Exit code: 0
   - Result: pass
   - Output: `all 10 B027 traceability records exist`

2. Prompt coverage check
   - Command: `rg -n "CODEX-0651|...|CODEX-0675" evidence/batches/BATCH-027/WORK_PLAN.md`
   - Exit code: 0
   - Result: pass
   - Evidence: all 25 B027 Prompt IDs are mapped in the work plan with a role, current-safe target, allowed scope, and test responsibility or non-code reason.

3. Current-safe traceability output scan
   - Command: PowerShell wrapper around `rg -n "source-archive|v5|v6|V5|V6|crates/trpg-data-eventing/src|crates/trpg-data-eventing/tests" <10 B027 traceability docs>`
   - Exit code: 0
   - Result: pass
   - Output: `no source-archive, old version token, or Rust output path in B027 traceability docs`

## B027 Primary Target Tests

1. `cargo test -p trpg-data-eventing --test adr_0002_event_sourcing_cqrs_contract_tests`
   - Exit code: 0
   - Result: pass
   - Tests: 3 passed, 0 failed
   - Coverage: CODEX-0661 current-safe module name, owner ID, contract registration, event append path, authority guard, idempotency guard, version guard, visibility label, fact provenance, projection checkpoint, replay eligibility.

2. `cargo test -p trpg-data-eventing --test adr_0005_postgres_pgvector_contract_tests`
   - Exit code: 0
   - Result: pass
   - Tests: 3 passed, 0 failed
   - Coverage: CODEX-0663 current-safe module name, owner ID, contract registration, event append path, authority guard, idempotency guard, version guard, visibility label, fact provenance, RAG/snapshot read-model scope, replay eligibility.

## S03 / Data Eventing Checks

1. `cargo fmt --all -- --check`
   - Exit code: 0
   - Result: pass
   - Note: Cargo emitted the existing Windows warning `could not canonicalize path C:\Users\zyc14588`.

2. `$env:CARGO_BUILD_JOBS='1'; cargo test -p trpg-data-eventing --all-features`
   - Exit code: 0
   - Result: pass
   - Tests included:
     - adr_0002_event_sourcing_cqrs_contract_tests: 3 passed
     - adr_0005_postgres_pgvector_contract_tests: 3 passed
     - batch_024_data_eventing_contract_tests: 8 passed
     - batch_025_data_eventing_contract_tests: 5 passed
     - batch_026_data_eventing_contract_tests: 5 passed
     - event_store_contract: 4 passed
     - projection_replay: 2 passed
     - doc tests: 0 tests
   - Note: Cargo emitted the existing Windows warning `could not canonicalize path C:\Users\zyc14588`.

3. `$env:CARGO_BUILD_JOBS='1'; cargo test -p trpg-data-eventing --test event_store_contract`
   - Exit code: 0
   - Result: pass
   - Tests: 4 passed, 0 failed

4. `$env:CARGO_BUILD_JOBS='1'; cargo test -p trpg-data-eventing --test projection_replay`
   - Exit code: 0
   - Result: pass
   - Tests: 2 passed, 0 failed

5. `$env:CARGO_BUILD_JOBS='1'; cargo clippy -p trpg-data-eventing --all-targets --all-features -- -D warnings`
   - Exit code: 0
   - Result: pass
   - Note: Cargo emitted the existing Windows warning `could not canonicalize path C:\Users\zyc14588`.

6. `$env:CARGO_BUILD_JOBS='1'; cargo test --workspace --all-features`
   - Exit code: 0
   - Result: pass
   - Coverage: workspace regression gate including trpg-data-eventing B027 target tests.

7. `$env:CARGO_BUILD_JOBS='1'; cargo clippy --workspace --all-targets --all-features -- -D warnings`
   - Exit code: 0
   - Result: pass
   - Note: Cargo emitted the existing Windows warning `could not canonicalize path C:\Users\zyc14588`.

## Fixture / Visibility Checks

1. `Test-Path fixtures/stages/S03_stage_acceptance_fixture.v1.json.md`
   - Exit code: 0
   - Result: pass
   - Output: `True`

2. `Test-Path fixtures/stages/detailed/S03_event_store_projection_hash.current.json.md`
   - Exit code: 0
   - Result: pass
   - Output: `True`

3. `rg -n "keeper_only|private_to_player|ai_internal" fixtures crates/trpg-data-eventing docs/codex/06-data-eventing evidence/batches/BATCH-027`
   - Exit code: 0
   - Result: pass as inspection input
   - Evidence: sensitive labels appear in fixture/test/policy contexts and are covered by redaction tests; no B027 player-visible output file was created.

4. `rg -n "player[-_ ]visible|player_visible|VisibilityLabel::Player|viewer.*player" crates/trpg-data-eventing/tests crates/trpg-data-eventing/src fixtures docs/codex/06-data-eventing`
   - Exit code: 0
   - Result: pass as inspection input
   - Evidence: player-visible references route through redaction tests, including `projection_replay_redacts_private_keeper_and_ai_internal_events` and `b024_redacts_private_keeper_and_ai_internal_from_player_visible_replay`.

## SQLx / Compose / pnpm Gate

1. `Get-ChildItem Env:DATABASE_URL`
   - Exit code: 1
   - Result: environment not configured
   - Output: PowerShell reported that `DATABASE_URL` does not exist in the environment.

2. `sqlx --version`
   - Exit code: 0
   - Result: pass
   - Output: `sqlx-cli 0.9.0`

3. `sqlx migrate run`
   - Exit code: 1
   - Result: not passed
   - Output: `error: --database-url or DATABASE_URL must be set`
   - Risk: live SQLx migration was not executed because no database URL is configured.

4. `sqlx migrate revert`
   - Exit code: 1
   - Result: not passed
   - Output: `error: --database-url or DATABASE_URL must be set`
   - Risk: live SQLx revert was not executed because no database URL is configured.

5. Docker / Podman availability checks
   - Commands: `where.exe docker`, `where.exe podman`, `Test-Path 'C:\Program Files\Docker\Docker\resources\bin\docker.exe'`, `Test-Path 'C:\Program Files\Podman\podman.exe'`, and Docker/Podman service scan.
   - Result: unavailable
   - Output: no Docker or Podman CLI, install path, or service was found, so a temporary PostgreSQL container could not be started from this shell.

6. WSL fallback checks
   - Command: `wsl.exe --list --quiet`
   - Result: unavailable
   - Output: no WSL distribution was listed, so WSL could not provide Docker or PostgreSQL.

7. PostgreSQL local runtime checks
   - Commands: `where.exe psql`, `where.exe postgres`, `where.exe pg_ctl`, `where.exe initdb`, `Test-Path 'C:\Program Files\PostgreSQL'`, and PostgreSQL service scan.
   - Result: unavailable
   - Output: no PostgreSQL CLI/runtime command, install path, or service was found in this shell.

8. `pnpm --version`
   - Exit code: 0
   - Result: pass
   - Output: `11.7.0`

9. `rg --files -g package.json -g pnpm-lock.yaml -g docker-compose.yml -g docker-compose.yaml -g compose.yml -g compose.yaml`
   - Exit code: 1
   - Result: not applicable
   - Output: no package manager or compose entrypoints found in this repository snapshot.

## Test Scope Note

CODEX-0661 and CODEX-0663 now have current-safe implementation evidence and dedicated target tests. Supplemental B027 rows remain prompt-only. Live SQLx migration run/revert remains blocked because this shell has no `DATABASE_URL`, Docker/Podman runtime, WSL distribution, or local PostgreSQL runtime. This evidence does not declare BATCH-027 PASS.
