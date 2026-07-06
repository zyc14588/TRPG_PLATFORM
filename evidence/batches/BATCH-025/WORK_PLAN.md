# BATCH-025-06-data-eventing Work Plan

Batch: `B025`
Scope: `06-data-eventing`
Stage: `S03-data-eventing-persistence`
Date: `2026-07-05`

## Governance Inputs Read

- `AGENTS.md`
- `CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`
- `SOURCE_BUNDLE_INTEGRATION_GUIDE.md`
- `docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md`
- `docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`
- `docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md`
- `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`
- `batches/B025.md`
- `stages/s03-data-eventing-persistence/{README.md,START_PROMPT.md,TEST_PLAN.md,TEST_DATA.md,ACCEPTANCE_PROMPT.md,REPAIR_PROMPT.md}`

## Batch Fact Reconciliation

The user supplied `primary prompt count: 0`. The authoritative batch file `batches/B025.md`, after applying the normalized execution map and current-safe output map, declares 25 prompts: 11 primary implementation prompts and 14 supplemental requirement prompts.

Resolution: follow the repository authority order and execute only the B025 outputs mapped by current-safe names. Supplemental prompts are treated as constraints for their owning primary prompt and do not create Rust outputs in this batch unless they point to an existing primary owner.

## Prompt Map

| Prompt ID | File | Role | Current-safe module/output | Allowed changes | Test responsibility |
| --- | --- | --- | --- | --- | --- |
| `CODEX-0601-06-DATA-EVENTING-6b440dcd4b` | `P0024.md` | primary | `crates/trpg-data-eventing/src/persistence_postgresql.rs` | Rust contract module only | B025 contract test covers module mapping, SQL governance, append path |
| `CODEX-0602-06-DATA-EVENTING-e1f4876553` | `P0027.md` | supplemental | primary owner `event_bus_nats` | No Rust output; merge NATS constraints into tests | B025 test asserts existing NATS owner remains governed |
| `CODEX-0603-06-DATA-EVENTING-dd4ec4ebfa` | `P0028.md` | primary | `crates/trpg-data-eventing/src/redis_presence.rs` | Rust contract module only | B025 contract test covers cache/read-model boundary |
| `CODEX-0604-06-DATA-EVENTING-2cd43712b5` | `P0029.md` | primary | `crates/trpg-data-eventing/src/nats_jet_stream.rs` | Rust contract module only | B025 contract test covers outbox/NATS boundary |
| `CODEX-0605-06-DATA-EVENTING-7aa50c4023` | `P0030.md` | primary | `crates/trpg-data-eventing/src/postgre_sql_sq_lx_pgvector.rs` | Rust contract module only | B025 contract test covers RAG index as read model |
| `CODEX-0606-06-DATA-EVENTING-96df5cfdb1` | `P0031.md` | primary | `crates/trpg-data-eventing/src/sqlx_migrations.rs` | Rust contract module only | B025 contract test covers migration descriptors |
| `CODEX-0607-06-DATA-EVENTING-2a432fa185` | `P0032.md` | primary | `crates/trpg-data-eventing/src/event_sourcing_snapshot_projection.rs` | Rust contract module only | B025 contract test covers snapshot/projection rebuild |
| `CODEX-0608-06-DATA-EVENTING-fdec7ddc4c` | `P0041.md` | supplemental | primary owner `redis_cache_presence` | No Rust output | Existing B024 owner plus B025 constraints |
| `CODEX-0609-06-DATA-EVENTING-6f17ea580b` | `P0038.md` | primary | `crates/trpg-data-eventing/src/schema.rs` | Rust contract module only | B025 contract test covers schema field requirements |
| `CODEX-0610-06-DATA-EVENTING-09b651f68d` | `P0033.md` | supplemental | primary owner `event_bus_nats` | No Rust output | Existing NATS contract remains tested |
| `CODEX-0611-06-DATA-EVENTING-377fc4c0d9` | `P0035.md` | supplemental | primary owner `event_store_projections` | No Rust output | Existing projection contract remains tested |
| `CODEX-0612-06-DATA-EVENTING-4c0e3600ee` | `P0036.md` | supplemental | primary owner `schema` | No Rust output | B025 schema test covers required fields |
| `CODEX-0613-06-DATA-EVENTING-88e2bf8def` | `P0040.md` | supplemental | primary owner `outbox_projection_workers` | No Rust output | Existing outbox worker contract remains tested |
| `CODEX-0614-06-DATA-EVENTING-7df4ae3e1e` | `P0039.md` | supplemental | primary owner `persistence_migrations` | No Rust output | Existing migration contract remains tested |
| `CODEX-0615-06-DATA-EVENTING-58af1867fc` | `P0037.md` | primary | `crates/trpg-data-eventing/src/readme.rs` | Rust contract module only | B025 contract test covers README traceability module |
| `CODEX-0616-06-DATA-EVENTING-0b28c2b885` | `P0034.md` | primary | `crates/trpg-data-eventing/src/snapshot.rs` | Rust contract module only | B025 contract test covers snapshot append path |
| `CODEX-0617-06-DATA-EVENTING-b93be54634` | `P0043.md` | supplemental | primary owner `redis_presence` | No Rust output | B025 redis presence module test coverage |
| `CODEX-0618-06-DATA-EVENTING-eb3812d6e7` | `P0044.md` | supplemental | primary owner `nats_jet_stream` | No Rust output | B025 NATS JetStream module test coverage |
| `CODEX-0619-06-DATA-EVENTING-33b260183e` | `P0042.md` | supplemental | primary owner `postgre_sql_sq_lx_pgvector` | No Rust output | B025 pgvector module test coverage |
| `CODEX-0620-06-DATA-EVENTING-f991a07544` | `P0045.md` | primary | `crates/trpg-data-eventing/src/event_command_json_schema.rs` | Rust contract module only | B025 test covers command/event schema fields |
| `CODEX-0621-06-DATA-EVENTING-c711ec9477` | `P0046.md` | supplemental | primary owner `outbox_projection_workers` | No Rust output | Existing outbox worker contract remains tested |
| `CODEX-0622-06-DATA-EVENTING-a09959c9b6` | `P0047.md` | supplemental | primary owner `persistence_migrations` | No Rust output | Existing migration contract remains tested |
| `CODEX-0623-06-DATA-EVENTING-75b9d3460e` | `P0048.md` | supplemental | primary owner `readme` | No Rust output | B025 README module test coverage |
| `CODEX-0624-06-DATA-EVENTING-41632cafef` | `P0049.md` | supplemental | primary owner `snapshot_strategy` | No Rust output | Existing snapshot strategy contract remains tested |
| `CODEX-0625-06-DATA-EVENTING-181b11b4cd` | `P0050.md` | primary | `crates/trpg-data-eventing/src/sqlx_migrations_contract.rs` | Rust contract module only | B025 test covers migration contract descriptors |

## Checks

Minimal relevant check:

- `cargo test -p trpg-data-eventing --test batch_025_data_eventing_contract_tests`

Stage checks:

- `cargo test -p trpg-data-eventing --all-features`
- `cargo test -p trpg-data-eventing --test event_store_contract`
- `cargo test -p trpg-data-eventing --test projection_replay`
