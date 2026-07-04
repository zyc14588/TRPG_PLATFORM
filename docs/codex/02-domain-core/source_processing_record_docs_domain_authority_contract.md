# Source Processing Record - source_processing_record_docs_domain_authority_contract

- Prompt ID: CODEX-0288-02-DOMAIN-CORE-e61cec44ff
- Batch: BATCH-009-02-domain-core
- Prompt file: codex-prompts/02-domain-core/P0063.md
- Source file: recorded in batches/B009.md for this prompt ID (provenance only)
- Current-safe module: domain_core::source_processing_record_docs_domain_authority_contract
- Current-safe output: docs/codex/02-domain-core/source_processing_record_docs_domain_authority_contract.md
- Allowed role: documentation-or-traceability

## Boundary

This record is traceability-only and does not authorize implementation output. Authority Contract behavior is implemented through current-safe domain-core modules and tests, not historical source-path names.

## Governance Carry-Forward

- Authority Contract creation locks mode, owner, and version for the campaign.
- Authority mutation is rejected in place; mode changes require a forked child campaign.
- AI actors cannot bypass the command envelope, workflow decision, rules/tool decision, event store, or visibility gates.

## Processing Result

The prompt was normalized to the current-safe Markdown output above and recorded for BATCH-009 traceability.
