# BATCH-025 Acceptance Evidence

Current-status boundary (2026-07-20): this document is interpreted together
with `evidence/stages/S03/p04-local-revalidation.txt` and
`docs/audit/p04/P04_FINAL_STATUS.md`. Earlier BATCH-025 transcripts remain
provenance only and do not establish the repaired P04 result.

Batch: `BATCH-025-06-data-eventing`
Mode: Strict Governance Final

## Implemented Scope

- Added current-safe primary modules for the 11 B025 implementation prompts:
  - `persistence_postgresql`
  - `redis_presence`
  - `nats_jet_stream`
  - `postgre_sql_sq_lx_pgvector`
  - `sqlx_migrations`
  - `event_sourcing_snapshot_projection`
  - `schema`
  - `readme`
  - `snapshot`
  - `event_command_json_schema`
  - `sqlx_migrations_contract`
- Registered B025 contracts through `batch_025_data_event_contracts()` and extended `all_data_event_contracts()`.
- Added B025 named command/event/service/repository/error artifacts through the existing data-eventing contract pattern.
- Kept all formal writes on `CommandEnvelope -> AuthorityContract -> EventStore -> Projection`.
- Did not add direct OpenAI, Ollama, llama.cpp, local model, business-layer LLM, direct agent DB write, or direct business formal-state write paths.
- Did not use `source-archive/**` as executable prompt material.

## Supplemental Prompt Handling

The 14 B025 supplemental prompts were treated as constraints only. They did not create Rust implementation outputs. Their NATS, Redis, projection, migration, schema, README, and snapshot constraints are covered by the owning primary modules or existing B024 owners.

## Tests Added Or Updated

- Added `crates/trpg-data-eventing/tests/batch_025_data_eventing_contract_tests.rs`.
- Updated the B024 aggregate test so it still verifies all B024 contracts while allowing later batch contracts to be appended to the global registry.
- Updated `crates/trpg-data-eventing/tests/projection_replay.rs` so S03 detailed fixture expected events, expected records, expected errors, and failure cases are executable assertions.
- Added a forward-only SQLx migration for leased Outbox delivery, materialized Projection recovery, audit v3 context binding and real pgvector RAG, then live-verified apply, repeated no-op apply, checksum protection and schema signatures against dedicated PostgreSQL primary/witness databases. Backup/restore is owned and executed separately by `trpg-ops`.

## Acceptance Notes

- Current-safe names are asserted for module, event, schema, NATS subject, metric, migration, and required column identifiers.
- `expected_version`, idempotency, visibility, fact provenance, authority contract validation, and direct-agent bypass denial are covered by tests.
- Projection, snapshot, and pgvector/RAG outputs remain rebuildable read models derived from Event Store evidence. The current RAG gate executes pgvector cosine search and database-enforced Visibility/Fact Provenance inheritance; it is not a marker-only contract.
- S03 projection replay hash is asserted as `sha256:6d967a6be23067c845a53f640c9d0092a3ec5ecfc2f88edb5c94bb4471def297` after the P04 envelope/hash-domain revalidation on 2026-07-20.
- Required S03 evidence exists under `docs/reports/stages/` and `evidence/stages/S03/`.

## SQLx Live Migration

Closed for the repaired P04 scope. `sqlx migrate info`, forward-only
`sqlx migrate run`, a repeated successful no-op apply, migration-upgrade tests,
and schema assertions were executed against dedicated PostgreSQL 18.4
primary/witness databases. Data/Eventing's Event Store integration also runs an embedded
backup—destroy—restore—rebuild drill; the separate `trpg-ops` target restored a custom archive into an
independent empty database and rejected manifest tampering. The current human-authored summary is
`evidence/stages/S03/p04-local-revalidation.txt`; final p00-4 machine evidence is
outside the repository at
`/tmp/p04-final-evidence-20260721/` with separate Data/Eventing and backup/restore
manifests, and was accepted by `verify_evidence_schema.py`.
The earlier run/revert/run transcript in `evidence/batches/BATCH-025/TEST_RESULTS.md`
is historical only.
