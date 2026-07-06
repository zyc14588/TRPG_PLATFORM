# BATCH-026 Test Results

Date: 2026-07-06

## Database Configuration

Temporary PostgreSQL was started only for the S03 SQLx migration gate.

- Image: `pgvector/pgvector:pg16`
- Container: `coc-ai-trpg-b026-postgres`
- `DATABASE_URL`: `postgres://trpg:trpg@127.0.0.1:15432/trpg_b026`
- Readiness command: `docker exec coc-ai-trpg-b026-postgres pg_isready -U trpg -d trpg_b026`
- Readiness exit code: 0
- Readiness output: `/var/run/postgresql:5432 - accepting connections`
- Cleanup command: `docker stop coc-ai-trpg-b026-postgres`
- Cleanup exit code: 0
- Cleanup output: `coc-ai-trpg-b026-postgres`

## Commands

1. `cargo fmt --all -- --check`
   - Exit code: 0
   - Result: pass
   - Note: Cargo emitted the existing Windows warning `could not canonicalize path C:\Users\zyc14588`.

2. `cargo clippy -p trpg-data-eventing --all-targets --all-features -- -D warnings`
   - Exit code: 0
   - Result: pass
   - Output summary: `Checking trpg-data-eventing v0.1.0`; `Finished dev profile`.

3. `cargo test -p trpg-data-eventing --test batch_026_data_eventing_contract_tests`
   - Exit code: 0
   - Result: pass
   - Tests: 5 passed, 0 failed
   - Covered tests:
     - `b026_contract_metadata_covers_api_nats_rag_cache_and_persistence`
     - `b026_appends_only_through_governed_event_store_path`
     - `b026_visibility_provenance_and_projection_replay_are_preserved`
     - `b026_primary_surfaces_append_governed_events_and_bind_fixtures`
     - `b026_primary_contracts_map_to_current_safe_outputs`

4. `$env:CARGO_BUILD_JOBS='1'; cargo test -p trpg-data-eventing --all-features`
   - Exit code: 0
   - Result: pass
   - Tests:
     - lib: 0 tests
     - batch_024_data_eventing_contract_tests: 8 passed
     - batch_025_data_eventing_contract_tests: 5 passed
     - batch_026_data_eventing_contract_tests: 5 passed
     - event_store_contract: 4 passed
     - projection_replay: 2 passed
     - doc tests: 0 tests

5. `cargo test -p trpg-data-eventing --test event_store_contract`
   - Exit code: 0
   - Result: pass
   - Tests: 4 passed, 0 failed
   - Fixture coverage: S03 stage fixture, detailed projection hash fixture, migration SQL text, RAG/realtime visibility fixture tokens.

6. `cargo test -p trpg-data-eventing --test projection_replay`
   - Exit code: 0
   - Result: pass
   - Tests: 2 passed, 0 failed
   - Fixture coverage: projection hash remains Event Store derived; private, keeper-only, and AI-internal events are redacted from player-visible replay.

7. `rg -n "keeper_only|private_to_player|ai_internal|REDACTED|projection_hash_stable|OutboxMessage" fixtures/stages/S03_stage_acceptance_fixture.v1.json.md fixtures/stages/detailed/S03_event_store_projection_hash.current.json.md fixtures/api/api_ws_nats_contract_cases.v1.json.md fixtures/rag/rag_snapshot_cases.v1.json.md crates/trpg-data-eventing/tests/event_store_contract.rs crates/trpg-data-eventing/tests/projection_replay.rs crates/trpg-data-eventing/tests/batch_026_data_eventing_contract_tests.rs`
   - Exit code: 0
   - Result: pass
   - Evidence: fixture tokens are bound to tests that assert redaction, projection hash stability, and outbox conversion. This scan is evidence discovery; leakage pass/fail is enforced by the cargo tests above.

8. `$env:DATABASE_URL='postgres://trpg:trpg@127.0.0.1:15432/trpg_b026'; sqlx migrate run`
   - Exit code: 0
   - Result: pass
   - Output: `Applied 20260705000100/migrate create data eventing event store (28.645ms)`

9. `$env:DATABASE_URL='postgres://trpg:trpg@127.0.0.1:15432/trpg_b026'; sqlx migrate revert --yes`
   - Exit code: 1
   - Result: unsupported CLI option, not counted as migration gate pass
   - Output: `error: unexpected argument '--yes' found`
   - Follow-up: `sqlx migrate revert --help` confirmed this sqlx version has no confirmation flag.

10. `$env:DATABASE_URL='postgres://trpg:trpg@127.0.0.1:15432/trpg_b026'; sqlx migrate revert`
    - Exit code: 0
    - Result: pass
    - Output: `Applied 20260705000100/revert create data eventing event store (6.5391ms)`

11. `$env:DATABASE_URL='postgres://trpg:trpg@127.0.0.1:15432/trpg_b026'; sqlx migrate run`
    - Exit code: 0
    - Result: pass
    - Output: `Applied 20260705000100/migrate create data eventing event store (25.2352ms)`

12. `$env:DATABASE_URL='postgres://trpg:trpg@127.0.0.1:15432/trpg_b026'; sqlx migrate run`
    - Exit code: 0
    - Result: pass
    - Output: no output; no pending migrations.

13. `Get-ChildItem -Name package.json, pnpm-lock.yaml, docker-compose.yml, docker-compose.yaml, compose.yml, compose.yaml -ErrorAction SilentlyContinue`
    - Exit code: 1
    - Result: not applicable
    - Reason: the command returned no package manager or compose entrypoints at repository root. Docker was still used directly for the SQLx database gate above.

## BATCH-026 Primary Prompt Evidence

| Prompt | Current-safe output | Implementation evidence | Target test or gate |
|---|---|---|---|
| P0051 / CODEX-0626 | `api_websocket_nats_contracts` | Appends through governed Event Store and binds API/WS/NATS fixture tokens, including private visibility and direct LLM deny subject evidence. | `b026_primary_surfaces_append_governed_events_and_bind_fixtures`; batch test command 3 |
| P0053 / CODEX-0627 | `nats_subjects` | Registers current-safe NATS subject metadata and appends it through the shared governed Event Store path. | `b026_primary_contracts_map_to_current_safe_outputs`; `b026_primary_surfaces_append_governed_events_and_bind_fixtures` |
| P0052 / CODEX-0628 | `nats_subject_contracts` | Registers NATS schema/contract metadata and keeps publish behavior derived from Event Store/outbox evidence. | `b026_contract_metadata_covers_api_nats_rag_cache_and_persistence`; `b026_primary_surfaces_append_governed_events_and_bind_fixtures` |
| P0055 / CODEX-0630 | `nats_subjects_source_contract` | Appends source-contract metadata with visibility and provenance preserved on the event payload. | `b026_primary_surfaces_append_governed_events_and_bind_fixtures` |
| P0057 / CODEX-0634 | `domain_event_sourcing_projection` | Projection is rebuilt from Event Store events and asserts replay sequence/hash behavior through S03 fixture-backed tests. | `b026_primary_surfaces_append_governed_events_and_bind_fixtures`; `projection_replay` command 6 |
| P0059 / CODEX-0635 | `rag_snapshot` | RAG snapshot metadata is event-derived and fixture-bound; player context remains redacted. | `b026_primary_surfaces_append_governed_events_and_bind_fixtures`; `event_store_contract` command 5 |
| P0061 / CODEX-0636 | `cache_redis_impl` | Cache is asserted as a derived read model, not canonical state; the clippy constant assertion uses `std::hint::black_box` while still checking `!CACHE_IS_CANONICAL`. | `b026_appends_only_through_governed_event_store_path`; `b026_primary_surfaces_append_governed_events_and_bind_fixtures` |
| P0062 / CODEX-0637 | `event_bus_nats_impl` | Event bus publish surface is converted to `OutboxMessage` from the governed event and checks `OutboxPublish` plus `OUTBOX_TABLE`. | `b026_primary_surfaces_append_governed_events_and_bind_fixtures` |
| P0063 / CODEX-0638 | `persistence_postgresql_impl` | Persistence surface is tied to Event Store/outbox/projection checkpoint metadata and live SQLx run/revert/run migration evidence. | `b026_primary_surfaces_append_governed_events_and_bind_fixtures`; SQLx commands 8, 10, 11, 12 |

## Stage Check

The S03 data-eventing persistence checks required by `stages/s03-data-eventing-persistence/TEST_PLAN.md` were run with crate-local package selection:

- `sqlx migrate run`: passed with `DATABASE_URL=postgres://trpg:trpg@127.0.0.1:15432/trpg_b026`.
- `cargo test -p trpg-data-eventing --all-features`: passed.
- `cargo test -p trpg-data-eventing --test event_store_contract`: passed.
- `cargo test -p trpg-data-eventing --test projection_replay`: passed.

No pnpm test was applicable because this repository root has no `package.json` or `pnpm-lock.yaml`. No docker compose test was applicable because this repository root has no compose file; Docker was used directly to provide the temporary PostgreSQL service for SQLx.
