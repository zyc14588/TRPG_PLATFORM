# BATCH-026 Work Plan

Batch: BATCH-026-06-data-eventing - Strict Governance Final

Authority context read before edits:

- AGENTS.md
- CODEX_STANDALONE_BOOTSTRAP_PROMPT.md
- SOURCE_BUNDLE_INTEGRATION_GUIDE.md
- docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md
- CODEX_MASTER_EXECUTION_GUIDE.md
- CODEX_START_ACCEPT_TEST_RELEASE_GUIDE.md
- CODEX_STRICT_OPERATION_CHECKLIST.md
- codex-operator-guides/README.md
- docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md
- docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md
- docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md
- batches/B026.md
- stages/s03-data-eventing-persistence/{README.md,START_PROMPT.md,TEST_PLAN.md,TEST_DATA.md,ACCEPTANCE_PROMPT.md,REPAIR_PROMPT.md}
- docs/codex/06-data-eventing/{AGENTS.md,README.md,per-file-prompt-manifest.md,codex-module-code-prompt.md,codex-module-test-prompt.md,codex-module-review-prompt.md,m_06_data_eventing.md}

Scope note:

- The user-provided batch fact says primary prompt count is 0.
- The active repository authority files for B026 map 25 prompts as 9 primary-implementation, 4 supplemental-requirement, and 12 documentation-or-traceability prompts.
- This plan follows the active normalized execution map and current-safe output map, without using source-archive names as current outputs.

## Prompt Map

| Prompt | Codex ID | Kind | Target | Allowed changes | Test responsibility |
|---|---|---|---|---|---|
| P0051 | CODEX-0626 | primary-implementation | crates/trpg-data-eventing/src/api_websocket_nats_contracts.rs | Create/update contract module and test-facing constants only | Contract registration, governed append, visibility/provenance replay |
| P0053 | CODEX-0627 | primary-implementation | crates/trpg-data-eventing/src/nats_subjects.rs | Create/update NATS subject module only | Current-safe subject names and required data-event subjects |
| P0052 | CODEX-0628 | primary-implementation | crates/trpg-data-eventing/src/nats_subject_contracts.rs | Create/update NATS contract module only | Contract fields, no historical names, outbox-derived publish path |
| P0054 | CODEX-0629 | supplemental-requirement | codex-prompts/06-data-eventing/P0054.md | No implementation output; source prompt already records merge target CODEX-0628 | Covered by CODEX-0628 tests |
| P0055 | CODEX-0630 | primary-implementation | crates/trpg-data-eventing/src/nats_subjects_source_contract.rs | Create/update source contract module only | Source contract subjects preserve provenance and visibility metadata |
| P0058 | CODEX-0631 | supplemental-requirement | codex-prompts/06-data-eventing/P0058.md | No implementation output; source prompt already records merge target CODEX-0059 | Covered by event bus/NATS tests |
| P0060 | CODEX-0632 | supplemental-requirement | codex-prompts/06-data-eventing/P0060.md | No implementation output; source prompt already records merge target CODEX-0057 | Covered by cache tests |
| P0056 | CODEX-0633 | supplemental-requirement | codex-prompts/06-data-eventing/P0056.md | No implementation output; source prompt already records merge target CODEX-0601 | Covered by persistence tests |
| P0057 | CODEX-0634 | primary-implementation | crates/trpg-data-eventing/src/domain_event_sourcing_projection.rs | Create/update projection contract module only | Projection remains rebuildable from Event Store |
| P0059 | CODEX-0635 | primary-implementation | crates/trpg-data-eventing/src/rag_snapshot.rs | Create/update RAG snapshot module and metadata constants only | RAG metadata includes source_type, visibility, version, owner, allowed_use |
| P0061 | CODEX-0636 | primary-implementation | crates/trpg-data-eventing/src/cache_redis_impl.rs | Create/update Redis cache implementation contract only | Cache is derived read model; no canon writes |
| P0062 | CODEX-0637 | primary-implementation | crates/trpg-data-eventing/src/event_bus_nats_impl.rs | Create/update NATS event bus implementation contract only | Outbox/retry/DLQ and derived publish path |
| P0063 | CODEX-0638 | primary-implementation | crates/trpg-data-eventing/src/persistence_postgresql_impl.rs | Create/update PostgreSQL implementation contract only | Event Store + outbox + projection checkpoint transaction surface |
| P0066 | CODEX-0639 | documentation-or-traceability | docs/codex/06-data-eventing/source_processing_record_docs_adr_adr_0002_event_sourcing_cqrs.md | Create/update traceability markdown only | Evidence references and policy assertions |
| P0065 | CODEX-0640 | documentation-or-traceability | docs/codex/06-data-eventing/source_processing_record_docs_adr_adr_0004_nats_jetstream.md | Create/update traceability markdown only | Evidence references and policy assertions |
| P0064 | CODEX-0641 | documentation-or-traceability | docs/codex/06-data-eventing/source_processing_record_docs_adr_adr_0005_postgres_pgvector.md | Create/update traceability markdown only | Evidence references and policy assertions |
| P0067 | CODEX-0642 | documentation-or-traceability | docs/codex/06-data-eventing/source_processing_record_docs_contracts_nats_subjects.md | Create/update traceability markdown only | Evidence references and policy assertions |
| P0068 | CODEX-0643 | documentation-or-traceability | docs/codex/06-data-eventing/source_processing_record_docs_db_sqlx_migrations_implementation.md | Create/update traceability markdown only | Evidence references and policy assertions |
| P0077 | CODEX-0644 | documentation-or-traceability | docs/codex/06-data-eventing/source_processing_record_docs_implementation_06_data_eventing_cache_redis.md | Create/update traceability markdown only | Evidence references and policy assertions |
| P0080 | CODEX-0645 | documentation-or-traceability | docs/codex/06-data-eventing/source_processing_record_docs_implementation_06_data_eventing_database_schema_index.md | Create/update traceability markdown only | Evidence references and policy assertions |
| P0074 | CODEX-0646 | documentation-or-traceability | docs/codex/06-data-eventing/source_processing_record_docs_implementation_06_data_eventing_event_bus_nats.md | Create/update traceability markdown only | Evidence references and policy assertions |
| P0072 | CODEX-0647 | documentation-or-traceability | docs/codex/06-data-eventing/source_processing_record_docs_implementation_06_data_eventing_event_store_projections.md | Create/update traceability markdown only | Evidence references and policy assertions |
| P0070 | CODEX-0648 | documentation-or-traceability | docs/codex/06-data-eventing/source_processing_record_docs_implementation_06_data_eventing_event_schema_index.md | Create/update traceability markdown only | Evidence references and policy assertions |
| P0076 | CODEX-0649 | documentation-or-traceability | docs/codex/06-data-eventing/source_processing_record_docs_implementation_06_data_eventing_outbox_projection_workers.md | Create/update traceability markdown only | Evidence references and policy assertions |
| P0073 | CODEX-0650 | documentation-or-traceability | docs/codex/06-data-eventing/source_processing_record_docs_implementation_06_data_eventing_persistence_migrations.md | Create/update traceability markdown only | Evidence references and policy assertions |

## Minimal Implementation Slice

1. Add B026 Rust modules mapped by current-safe output names.
2. Register B026 primary contracts in the data-eventing crate.
3. Add focused B026 tests for current-safe naming, governed append path, visibility/provenance replay, NATS subject metadata, RAG metadata, cache derivation, event bus outbox/DLQ metadata, and PostgreSQL persistence contract fields.
4. Add documentation traceability records for the 12 documentation-or-traceability prompts.
5. Run the smallest relevant B026 test first, then run S03 data-eventing checks.
