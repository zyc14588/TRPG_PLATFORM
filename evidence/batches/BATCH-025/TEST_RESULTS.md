# BATCH-025 Test Results

Date: `2026-07-05`
Scope: `trpg-data-eventing`
Mode: strict repair verification

## Fixture Assertion Repair

`fixtures/stages/detailed/S03_event_store_projection_hash.current.json.md` is now bound to executable assertions:

- `ProjectionRebuilt.hash` is asserted against the rebuilt projection hash: `sha256:a83861bce178f274e6a2e809c790770577445268b48fedfb889af4b87f8c1c50`.
- `expected_records.OutboxMessage` is asserted through `OutboxMessage { event_id, correlation_id, causation_id }`.
- `expected_records.ProjectionCheckpoint` is asserted through `ProjectionCheckpoint { stream_id, version, projection_hash }`.
- `expected_errors.wrong_expected_version` is asserted from a real stale append.
- `expected_errors.duplicate_idempotency_key` is asserted from a real idempotency replay.
- `failure_cases.mutable_event_update` is asserted by denying a direct business write and verifying the Event Store length does not change.

## Rust Commands

### `cargo test -p trpg-data-eventing --test projection_replay --test event_store_contract`

```text
running 4 tests
test migration_entry_is_repeatable_sqlx_evidence ... ok
test rag_and_realtime_fixtures_preserve_metadata_and_private_visibility ... ok
test event_store_contract_enforces_version_idempotency_and_visibility ... ok
test s03_fixtures_are_bound_to_event_store_contract_assertions ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 2 tests
test projection_replay_redacts_private_keeper_and_ai_internal_events ... ok
test projection_replay_hash_is_stable_and_event_store_derived ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

warn: could not canonicalize path C:\Users\zyc14588
   Compiling trpg-data-eventing v0.1.0 (C:\Users\zyc14588\coc_ai_trpg\crates\trpg-data-eventing)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.88s
     Running tests\event_store_contract.rs (target\debug\deps\event_store_contract-7652b0bd96035808.exe)
     Running tests\projection_replay.rs (target\debug\deps\projection_replay-539f56a74f7c6b34.exe)
```

### `cargo test -p trpg-data-eventing --test event_store_contract`

```text
running 4 tests
test rag_and_realtime_fixtures_preserve_metadata_and_private_visibility ... ok
test migration_entry_is_repeatable_sqlx_evidence ... ok
test event_store_contract_enforces_version_idempotency_and_visibility ... ok
test s03_fixtures_are_bound_to_event_store_contract_assertions ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

warn: could not canonicalize path C:\Users\zyc14588
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.30s
     Running tests\event_store_contract.rs (target\debug\deps\event_store_contract-7652b0bd96035808.exe)
```

### `cargo test -p trpg-data-eventing --test projection_replay`

```text
running 2 tests
test projection_replay_redacts_private_keeper_and_ai_internal_events ... ok
test projection_replay_hash_is_stable_and_event_store_derived ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

warn: could not canonicalize path C:\Users\zyc14588
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.22s
     Running tests\projection_replay.rs (target\debug\deps\projection_replay-539f56a74f7c6b34.exe)
```

### `cargo test -p trpg-data-eventing --test batch_025_data_eventing_contract_tests`

```text
running 5 tests
test b025_domain_artifacts_are_named_and_schema_bound ... ok
test b025_appends_only_through_governed_event_store_path ... ok
test b025_schema_and_migration_contracts_preserve_required_metadata ... ok
test b025_projection_snapshot_and_rag_outputs_are_rebuildable_read_models ... ok
test b025_primary_contracts_map_to_current_safe_outputs ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

warn: could not canonicalize path C:\Users\zyc14588
   Compiling trpg-data-eventing v0.1.0 (C:\Users\zyc14588\coc_ai_trpg\crates\trpg-data-eventing)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.87s
     Running tests\batch_025_data_eventing_contract_tests.rs (target\debug\deps\batch_025_data_eventing_contract_tests-9ddd7f8a78b4440b.exe)
```

### `cargo test -j 1 -p trpg-data-eventing --all-features`

```text
running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 8 tests
test b024_blocks_direct_agent_business_and_authority_contract_bypass ... ok
test b024_appends_formal_events_through_contract_and_preserves_replay_metadata ... ok
test b024_declares_current_safe_sqlx_migration_contract ... ok
test b024_declares_required_command_event_schema_fields ... ok
test b024_contracts_map_all_primary_prompts_to_current_safe_outputs ... ok
test b024_enforces_expected_version_and_idempotency_for_event_store_canon ... ok
test b024_redacts_private_keeper_and_ai_internal_from_player_visible_replay ... ok
test b024_rejects_legacy_or_source_derived_names ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 5 tests
test b025_domain_artifacts_are_named_and_schema_bound ... ok
test b025_appends_only_through_governed_event_store_path ... ok
test b025_projection_snapshot_and_rag_outputs_are_rebuildable_read_models ... ok
test b025_schema_and_migration_contracts_preserve_required_metadata ... ok
test b025_primary_contracts_map_to_current_safe_outputs ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 4 tests
test migration_entry_is_repeatable_sqlx_evidence ... ok
test s03_fixtures_are_bound_to_event_store_contract_assertions ... ok
test event_store_contract_enforces_version_idempotency_and_visibility ... ok
test rag_and_realtime_fixtures_preserve_metadata_and_private_visibility ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 2 tests
test projection_replay_redacts_private_keeper_and_ai_internal_events ... ok
test projection_replay_hash_is_stable_and_event_store_derived ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

warn: could not canonicalize path C:\Users\zyc14588
   Compiling trpg-data-eventing v0.1.0 (C:\Users\zyc14588\coc_ai_trpg\crates\trpg-data-eventing)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1.03s
     Running unittests src\lib.rs (target\debug\deps\trpg_data_eventing-3437977df5eb0a14.exe)
     Running tests\batch_024_data_eventing_contract_tests.rs (target\debug\deps\batch_024_data_eventing_contract_tests-a05c1acec54d0b4e.exe)
     Running tests\batch_025_data_eventing_contract_tests.rs (target\debug\deps\batch_025_data_eventing_contract_tests-9ddd7f8a78b4440b.exe)
     Running tests\event_store_contract.rs (target\debug\deps\event_store_contract-7652b0bd96035808.exe)
     Running tests\projection_replay.rs (target\debug\deps\projection_replay-539f56a74f7c6b34.exe)
   Doc-tests trpg_data_eventing
```

### `cargo fmt --all -- --check`

```text
warn: could not canonicalize path C:\Users\zyc14588
```

### `cargo clippy -p trpg-data-eventing --all-features -- -D warnings`

```text
warn: could not canonicalize path C:\Users\zyc14588
    Checking trpg-data-eventing v0.1.0 (C:\Users\zyc14588\coc_ai_trpg\crates\trpg-data-eventing)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.94s
```

## SQLx Live Migration Evidence

Disposable database:

```text
image: pgvector/pgvector:pg16
container: b025-sqlx-postgres
database: b025_acceptance
mapped port: 127.0.0.1:62171
DATABASE_URL: postgres://postgres:<redacted>@127.0.0.1:62171/b025_acceptance
```

Initial fixed-port Docker attempt failed because Windows rejected binding port 55432; the successful verification used Docker's random local port mapping.

### `docker run -d --rm --name b025-sqlx-postgres ... -p 127.0.0.1::5432 pgvector/pgvector:pg16`

```text
2354848b0a0292ee5ad6bee5f2cd9065fd2d7c14138f212c6ff6b2c2d95946cc
```

### `docker port b025-sqlx-postgres 5432/tcp`

```text
127.0.0.1:62171
```

### `docker exec b025-sqlx-postgres pg_isready -U postgres -d b025_acceptance`

```text
/var/run/postgresql:5432 - accepting connections
```

### `sqlx migrate run`

```text
Applied 20260705000100/migrate create data eventing event store (26.3997ms)
```

### `sqlx migrate revert`

```text
Applied 20260705000100/revert create data eventing event store (9.273ms)
```

### `sqlx migrate run`

```text
Applied 20260705000100/migrate create data eventing event store (26.625ms)
```

### Schema query

```text
      table_name       |     column_name      
-----------------------+----------------------
 event_outbox          | causation_id
 event_outbox          | correlation_id
 event_outbox          | event_id
 event_outbox          | event_sequence
 event_outbox          | idempotency_key
 event_outbox          | visibility_label
 event_store           | causation_id
 event_store           | correlation_id
 event_store           | expected_version
 event_store           | fact_provenance_kind
 event_store           | idempotency_key
 event_store           | visibility_label
 projection_checkpoint | projection_hash
 projection_checkpoint | stream_id
 projection_checkpoint | version
(15 rows)
```

### `docker stop b025-sqlx-postgres`

```text
b025-sqlx-postgres
```

## Result

PASS for BATCH-025 repair verification. Live SQLx migration run/revert/run was executed against PostgreSQL, and fixture hash/record/error/failure assertions are executable Rust tests.
