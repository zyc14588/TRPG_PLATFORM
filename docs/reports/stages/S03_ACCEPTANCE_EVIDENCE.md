# S03 Acceptance Evidence

Date: `2026-07-05`
Scope: `BATCH-025-06-data-eventing`
Conclusion: `PASS`

## Evidence

- Event Store remains canon: `append_data_event` validates `AuthorityContract` and appends through `EventStore`; direct business mutation is denied in `projection_replay_hash_is_stable_and_event_store_derived`.
- Projection/RAG/cache remain rebuildable read models: `rebuild_projection_from_events` derives `sha256:a83861bce178f274e6a2e809c790770577445268b48fedfb889af4b87f8c1c50` from Event Store events.
- Outbox/projection expected records are executable assertions: `OutboxMessage { event_id, correlation_id, causation_id }` and `ProjectionCheckpoint { stream_id, version, projection_hash }`.
- SQLx migration is live-verified against PostgreSQL with run/revert/run.

## Commands

- `cargo test -p trpg-data-eventing --test projection_replay --test event_store_contract`
- `cargo test -p trpg-data-eventing --test batch_025_data_eventing_contract_tests`
- `cargo test -j 1 -p trpg-data-eventing --all-features`
- `cargo fmt --all -- --check`
- `cargo clippy -p trpg-data-eventing --all-features -- -D warnings`
- `sqlx migrate run`
- `sqlx migrate revert`
- `sqlx migrate run`

Detailed outputs are in `evidence/batches/BATCH-025/TEST_RESULTS.md`, `evidence/stages/S03/event-store-contract.txt`, and `evidence/stages/S03/projection-replay-hash.txt`.
