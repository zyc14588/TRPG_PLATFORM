# BATCH-028 Work Plan

Batch: `BATCH-028-06-data-eventing`
Stage: `S03`
Date: 2026-07-07

## Read Inputs

- `AGENTS.md`
- `CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`
- `SOURCE_BUNDLE_INTEGRATION_GUIDE.md`
- `docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md`
- `docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`
- `docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md`
- `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`
- `docs/codex/00-index/codex-persistent-context.md`
- `docs/codex/00-index/codex-prompt-boundary.md`
- `stages/s03-data-eventing-persistence/*`
- `docs/codex/06-data-eventing/{AGENTS.md,README.md,m_06_data_eventing.md,per-file-prompt-manifest.md}`
- `batches/B028.md`
- `codex-prompts/06-data-eventing/P0101.md` through `P0107.md`
- S03 fixtures: `fixtures/event_store/golden_event_stream_expected.v1.json.md`, `fixtures/rag/rag_snapshot_cases.v1.json.md`, `fixtures/stages/detailed/S03_event_store_projection_hash.current.json.md`

## Scope Note

The user-provided fact says primary prompt count is 0, but local authority files `batches/B028.md` and `docs/codex/06-data-eventing/per-file-prompt-manifest.md` declare `CODEX-0682-06-DATA-EVENTING-af0d5b5090` as `primary-implementation`. This batch execution follows the repository authority and treats only that prompt as owning Rust output.

## Prompt Map

| Prompt ID | File | Role | Current-safe target | Allowed changes | Test responsibility |
|---|---|---|---|---|---|
| `CODEX-0676-06-DATA-EVENTING-02c8180880` | `codex-prompts/06-data-eventing/P0101.md` | supplemental | `data_eventing::cache_redis_impl` | No Rust/test output. Constraints remain supplemental to primary `CODEX-0636-06-DATA-EVENTING-2e55e84997`. | Covered by existing B026 cache tests; no new B028 test owner. |
| `CODEX-0677-06-DATA-EVENTING-3ad378a7dd` | `codex-prompts/06-data-eventing/P0102.md` | supplemental | `data_eventing::event_bus_nats_impl` | No Rust/test output. Constraints remain supplemental to primary `CODEX-0637-06-DATA-EVENTING-745a12af17`. | Covered by existing B026 NATS impl tests; no new B028 test owner. |
| `CODEX-0678-06-DATA-EVENTING-79a0f572c9` | `codex-prompts/06-data-eventing/P0103.md` | supplemental | `data_eventing::persistence_postgresql_impl` | No Rust/test output. Constraints remain supplemental to primary `CODEX-0638-06-DATA-EVENTING-0f91c8671e`. | Covered by existing B026 persistence impl tests; no new B028 test owner. |
| `CODEX-0679-06-DATA-EVENTING-20bc01add4` | `codex-prompts/06-data-eventing/P0104.md` | supplemental | `data_eventing::cache_redis` | No Rust/test output. Constraints remain supplemental to primary `CODEX-0057-06-DATA-EVENTING-069ff7204e`. | Covered by existing B024 cache tests; no new B028 test owner. |
| `CODEX-0680-06-DATA-EVENTING-6f2b9615a9` | `codex-prompts/06-data-eventing/P0105.md` | supplemental | `data_eventing::event_bus_nats` | No Rust/test output. Constraints remain supplemental to primary `CODEX-0059-06-DATA-EVENTING-8ceec1d689`. | Covered by existing B024 NATS tests; no new B028 test owner. |
| `CODEX-0681-06-DATA-EVENTING-3ecf12af00` | `codex-prompts/06-data-eventing/P0106.md` | supplemental | `data_eventing::persistence_postgresql` | No Rust/test output. Constraints remain supplemental to primary `CODEX-0601-06-DATA-EVENTING-6b440dcd4b`. | Covered by existing B025 persistence tests; no new B028 test owner. |
| `CODEX-0682-06-DATA-EVENTING-af0d5b5090` | `codex-prompts/06-data-eventing/P0107.md` | primary | `crates/trpg-data-eventing/src/event_json_schema.rs`; `crates/trpg-data-eventing/tests/event_json_schema_contract_tests.rs` | Add flat Rust module, export it from `lib.rs`, add contract tests. No migration, API handler, NATS subject, or direct LLM path. | Must prove required command/event schema fields, governed Event Store append, authority/write-path denials, visibility/provenance replay, and fixture binding. |

## Planned Commands

1. Minimal related check: `cargo test -p trpg-data-eventing --test event_json_schema_contract_tests --all-features`
2. Stage crate check: `cargo test -p trpg-data-eventing --all-features`
3. Formatting check if Rust files change: `cargo fmt --all -- --check`

## Non-Scope

- No `source-archive/**` executable input.
- No later batch start.
- No SQLx migration, API handler, NATS subject, model provider, Agent runtime, or Authority Contract changes.
