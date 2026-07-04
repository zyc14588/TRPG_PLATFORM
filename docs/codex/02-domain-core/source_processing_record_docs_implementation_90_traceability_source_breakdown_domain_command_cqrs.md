# Source Processing Record - Domain Command CQRS

Batch: B010  
Prompt ID: CODEX-0304-02-DOMAIN-CORE-3c0ffb8f8f  
Role: documentation-or-traceability  
Current-safe output: docs/codex/02-domain-core/source_processing_record_docs_implementation_90_traceability_source_breakdown_domain_command_cqrs.md

## Boundary

- This record is Markdown-only traceability maintenance.
- It does not own Rust src/test output, migrations, API handlers, NATS subjects, metric labels, workflows, or event schemas.
- Source path, source hash, and older version markers in the prompt remain provenance only.

## Normalized Anchors

- Formal writes stay on CommandEnvelope -> Workflow/Rules/Tool Decision -> EventStore -> Projection.
- Command metadata must include idempotency_key, expected_version, actor, authority_mode, visibility, fact_provenance, correlation_id, and causation_id.
- Existing domain command code keeps using `DomainAuthorityContract::validate_command` before appending `CommandAccepted` through `EventStore`.
- Projection, cache, summary, and RAG outputs remain rebuildable read models, not canon.

## B010 Application

- CODEX-0304 contributes no business Rust output in this batch.
- B010 primary implementation coverage for command append behavior is exercised through the current-safe modules and contract tests listed in `evidence/batches/BATCH-010/BATCH_WORK_PLAN.md`.
- No API, SQLx, or realtime layer was introduced from this traceability prompt.
