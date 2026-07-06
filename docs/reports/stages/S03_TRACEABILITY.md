# S03 Traceability

Date: `2026-07-05`
Batch: `BATCH-025-06-data-eventing`

## Fixture Binding

| Fixture item | Executable assertion |
| --- | --- |
| `ProjectionRebuilt.hash` | `projection_replay_hash_is_stable_and_event_store_derived` compares rebuilt projection hash to `sha256:a83861bce178f274e6a2e809c790770577445268b48fedfb889af4b87f8c1c50`. |
| `OutboxMessage.required_fields` | `OutboxMessage::from(event)` asserts `event_id`, `correlation_id`, and `causation_id`. |
| `ProjectionCheckpoint.required_fields` | `ProjectionCheckpoint::from_snapshot` asserts `stream_id`, `version`, and `projection_hash`. |
| `wrong_expected_version` | Real append with stale expected version maps to `EVENT_STREAM_VERSION_CONFLICT`. |
| `duplicate_idempotency_key` | Real idempotency replay maps to `IDEMPOTENCY_REPLAYED`. |
| `mutable_event_update` | Direct business write is denied and Event Store length remains unchanged, mapped to `EVENT_STORE_APPEND_ONLY`. |

## Prompt Coverage

B025 contains 25 rows: 11 primary implementation prompts and 14 supplemental requirement prompts. The primary prompts are represented by `batch_025_data_event_contracts()` and `batch_025_data_eventing_contract_tests`; supplemental prompts remain constraints on their owning primary modules.

## Migration Trace

`migrations/20260705000100_create_data_eventing_event_store.up.sql` and `.down.sql` were live-tested with SQLx against disposable PostgreSQL. The live schema includes Event Store metadata, Outbox `event_id/correlation_id/causation_id`, and ProjectionCheckpoint `stream_id/version/projection_hash`.
