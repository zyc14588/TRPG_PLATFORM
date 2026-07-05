# M 06 Data Eventing

Batch: BATCH-024-06-data-eventing

This document is the current-safe traceability record for `data_eventing::m_06_data_eventing`.
It records the B024 implementation boundary for the `trpg-data-eventing` crate.

## Current Boundary

- Event Store is canon. Projection, cache, outbox, NATS delivery, RAG snapshots, and summary views are rebuildable read models.
- Formal writes require `CommandEnvelope` fields for idempotency, expected version, actor, authority mode, authority contract version, visibility, fact provenance, correlation id, causation id, and formal write path.
- Authority Contract is immutable. HUMAN_KP and AI_KP remain campaign-level exclusive modes.
- Agent, provider, frontend, cache, projection worker, and business code cannot write formal state directly.
- NATS subjects and metrics use current-safe names only: `trpg.events.appended`, `trpg.projection.rebuild.requested`, `trpg_command_total`, `trpg_event_append_latency_ms`, `trpg_policy_deny_total`, `trpg_projection_lag_events`, and `trpg_visibility_redaction_total`.

## B024 Implemented Modules

- `data_eventing::cache_redis`
- `data_eventing::database_schema_index`
- `data_eventing::event_bus_nats`
- `data_eventing::event_schema_index`
- `data_eventing::event_store_projections`
- `data_eventing::outbox_projection_workers`
- `data_eventing::persistence_migrations`
- `data_eventing::snapshot_strategy`
- `data_eventing::adr_0002_event_sourcing_cqrs_event_sourcing_cqrs`
- `data_eventing::adr_0004_nats_jetstream`
- `data_eventing::adr_0005_postgres_pgvector_postgre_sql_pgvector`
- `data_eventing::adr_0010_rag_snapshot_rag_snapshot`
- `data_eventing::event_json_schema_source_contract`
- `data_eventing::event_store_sqlx_outbox_projection`
- `data_eventing::redis_cache_presence`

## Non-Goals

- `source-archive/**` remains provenance only.
- Historical source paths, old version tokens, hash fragments, and generated source names are not current module, migration, event, metric, NATS, workflow, or output names.
- B024 does not start later `06-data-eventing` prompt rows outside `batches/B024.md`.
