# Source Processing Record - Domain Authority Contract

Batch: B010  
Prompt ID: CODEX-0305-02-DOMAIN-CORE-4544e9633e  
Role: documentation-or-traceability  
Current-safe output: docs/codex/02-domain-core/source_processing_record_docs_implementation_90_traceability_source_breakdown_domain_authority_contract.md

## Boundary

- This record is Markdown-only traceability maintenance.
- It does not own Rust src/test output, migrations, API handlers, NATS subjects, metric labels, workflows, or event schemas.
- Source path, source hash, and older version markers in the prompt remain provenance only.

## Normalized Anchors

- Authority Contract is campaign-level, locked after creation, and changed only by fork.
- HUMAN_KP and AI_KP are mutually exclusive authority modes for a campaign.
- In-place authority changes must return an immutable-authority error and append no canon event.
- A fork may create a child Authority Contract while leaving the parent unchanged.

## B010 Application

- CODEX-0305 contributes no business Rust output in this batch.
- The current-safe primary authority module is `crates/trpg-domain-core/src/adr_0003_authority_contract.rs`.
- Contract tests assert rejection-without-event, direct-agent write rejection, fork-only change policy, and visibility/provenance replay behavior.
