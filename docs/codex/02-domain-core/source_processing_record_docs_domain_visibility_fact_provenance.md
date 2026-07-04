# Source Processing Record - source_processing_record_docs_domain_visibility_fact_provenance

- Prompt ID: CODEX-0292-02-DOMAIN-CORE-df5328e594
- Batch: BATCH-009-02-domain-core
- Prompt file: codex-prompts/02-domain-core/P0062.md
- Source file: recorded in batches/B009.md for this prompt ID (provenance only)
- Current-safe module: domain_core::source_processing_record_docs_domain_visibility_fact_provenance
- Current-safe output: docs/codex/02-domain-core/source_processing_record_docs_domain_visibility_fact_provenance.md
- Allowed role: documentation-or-traceability

## Boundary

This record is traceability-only. It does not permit weakening visibility checks, redaction behavior, or fact-source validation.

## Governance Carry-Forward

- Visibility labels must be enforced for agent context, tool results, RAG, summary, export, replay, logs, and metrics.
- Agent drafts, NPC claims, and player inferences cannot be promoted to confirmed facts without a confirmable event-backed source.
- Redaction decisions must be conservative for player-facing and export-facing derived objects.

## Processing Result

The prompt was normalized to the current-safe Markdown output above and recorded for BATCH-009 traceability.
