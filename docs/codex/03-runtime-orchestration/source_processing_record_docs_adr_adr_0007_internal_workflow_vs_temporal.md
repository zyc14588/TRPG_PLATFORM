# Source Processing Record

- Prompt ID: `CODEX-0393-03-RUNTIME-ORCHESTRATION-bce0a108cd`
- Current-safe module: `runtime_orchestration::source_processing_record_docs_adr_adr_0007_internal_workflow_vs_temporal`
- Current output: `docs/codex/03-runtime-orchestration/source_processing_record_docs_adr_adr_0007_internal_workflow_vs_temporal.md`
- Role: `documentation-or-traceability`

This record preserves the internal workflow versus external workflow governance decision for S06. Runtime remains an internal, auditable workflow layer; formal state still flows through Command, Workflow, Decision, Event Store, and Projection.

No Rust source, test, migration, API handler, event schema, NATS subject, metric, or formal write path is owned by this record.
