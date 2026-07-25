# BATCH-026 Acceptance Summary

> Historical batch boundary: the original B026 notes below are provenance.
> Current P04 acceptance is forward-only and is recorded in
> `docs/audit/p04/P04_TEST_RESULTS.md`; it includes real PostgreSQL, NATS 2.10
> JetStream and Redis execution.

Batch: BATCH-026-06-data-eventing - Strict Governance Final
Stage: S03 data-eventing persistence

## Implemented Scope

- Added 9 current-safe primary data-eventing modules for B026:
  - api_websocket_nats_contracts
  - nats_subjects
  - nats_subject_contracts
  - nats_subjects_source_contract
  - domain_event_sourcing_projection
  - rag_snapshot
  - cache_redis_impl
  - event_bus_nats_impl
  - persistence_postgresql_impl
- Registered the B026 primary contracts through `batch_026_data_event_contracts()` and `all_data_event_contracts()`.
- Added B026-focused contract tests.
- Added 12 documentation-or-traceability records under `docs/codex/06-data-eventing/`.
- Preserved the 4 supplemental prompts as prompt-only inputs; no extra implementation outputs were created for them.

## Governance Evidence

- Business/API/agent-facing surfaces still append formal facts only through the shared governed Event Store path.
- Direct agent writes, wrong expected_version, duplicate idempotency keys, and authority actor violations are rejected.
- Visibility and fact provenance are preserved across replay and projection rebuild.
- RAG snapshot metadata includes source_type, visibility, copyright_status, version, owner, allowed_use, fact_provenance, source_event_sequence, and chunk_hash; P04 later hardened this into the concrete pgvector table and query path.
- Cache, NATS, projection, and RAG surfaces are declared as rebuildable or derived from Event Store/outbox sources.
- Current-safe name checks pass through `b026_primary_contracts_map_to_current_safe_outputs`; historical source/path/version tokens are retained only as source provenance or deny-list values, not as current module/output names.

## Primary Prompt Evidence

| Prompt | Current-safe output | Evidence |
|---|---|---|
| P0051 / CODEX-0626 | `api_websocket_nats_contracts` | Governed Event Store append plus API/WS/NATS fixture token assertions. |
| P0053 / CODEX-0627 | `nats_subjects` | Current-safe NATS subject registration plus governed append assertion. |
| P0052 / CODEX-0628 | `nats_subject_contracts` | NATS schema metadata plus outbox-derived publish contract assertion. |
| P0055 / CODEX-0630 | `nats_subjects_source_contract` | Source-contract subject metadata with visibility/provenance assertion. |
| P0057 / CODEX-0634 | `domain_event_sourcing_projection` | Projection rebuild from Event Store and S03 projection replay fixture assertion. |
| P0059 / CODEX-0635 | `rag_snapshot` | RAG metadata and player-context redaction fixture assertion. |
| P0061 / CODEX-0636 | `cache_redis_impl` | Derived cache assertion, including `!CACHE_IS_CANONICAL` without a clippy constant assertion. |
| P0062 / CODEX-0637 | `event_bus_nats_impl` | `OutboxMessage` conversion from governed event and `OutboxPublish` assertion. |
| P0063 / CODEX-0638 | `persistence_postgresql_impl` | Event Store/outbox/projection checkpoint metadata; current P04 evidence uses forward-only migration verification. |

## Evidence Files

- evidence/batches/BATCH-026/WORK_PLAN.md
- evidence/batches/BATCH-026/TEST_RESULTS.md
- evidence/batches/BATCH-026/ACCEPTANCE_SUMMARY.md

## Historical Open Risks

- A prior parallel all-features run hit a Windows linker/file-lock error. The same check passed with `CARGO_BUILD_JOBS=1`.
- In the original B026 run, PostgreSQL was started only for the SQLx gate and Redis/NATS were not started. P04 later closed that evidence gap with real NATS 2.10 JetStream and Redis integration tests; this historical limitation is not a statement about current P04 execution.

## Next Batch Handoff

- Continue from the registered B026 contracts and evidence files.
- Do not treat source-archive paths as current module, migration, event, metric, workflow, test, or output names.
- Keep subsequent batch work behind Command -> Workflow -> Decision -> Event Store -> Projection and Agent Gateway boundaries.
