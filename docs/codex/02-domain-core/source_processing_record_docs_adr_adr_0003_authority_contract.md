# Source Processing Record - source_processing_record_docs_adr_adr_0003_authority_contract

- Prompt ID: CODEX-0287-02-DOMAIN-CORE-1da26d2a42
- Batch: BATCH-009-02-domain-core
- Prompt file: codex-prompts/02-domain-core/P0059.md
- Source file: recorded in batches/B009.md for this prompt ID (provenance only)
- Current-safe module: domain_core::source_processing_record_docs_adr_adr_0003_authority_contract
- Current-safe output: docs/codex/02-domain-core/source_processing_record_docs_adr_adr_0003_authority_contract.md
- Allowed role: documentation-or-traceability

## Boundary

This record does not authorize Rust src/, tests, migrations, API handlers, NATS subjects, workflows, event schemas, metric labels, or model-provider access. Historical version markers, source paths, prompt hashes, and SHA values are retained only as provenance and are not current product names.

## Governance Carry-Forward

- Authority Contract remains immutable after campaign creation and can change only through fork lineage.
- HUMAN_KP and AI_KP remain mutually exclusive campaign authority modes.
- Formal state writes remain Command -> Workflow -> Decision -> Event Store -> Projection.
- Event Store remains canonical; projections, caches, RAG indexes, and summaries remain rebuildable read models.
- Visibility labels and fact provenance must propagate through events, replay, summaries, exports, logs, and metrics.

## Processing Result

The prompt was normalized to the current-safe Markdown output above and recorded for BATCH-009 traceability.
