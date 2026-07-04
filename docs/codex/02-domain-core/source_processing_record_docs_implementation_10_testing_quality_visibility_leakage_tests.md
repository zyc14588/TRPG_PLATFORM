# Source Processing Record - Visibility Leakage Tests

Batch: B010  
Prompt ID: CODEX-0307-02-DOMAIN-CORE-e3936e4139  
Role: documentation-or-traceability  
Current-safe output: docs/codex/02-domain-core/source_processing_record_docs_implementation_10_testing_quality_visibility_leakage_tests.md

## Boundary

- This record is Markdown-only traceability maintenance.
- It does not own Rust src/test output, migrations, API handlers, NATS subjects, metric labels, workflows, or event schemas.
- Source path, source hash, and older version markers in the prompt remain provenance only.

## Normalized Anchors

- Negative tests must assert that unauthorized or visibility-ineligible principals receive no restricted event replay.
- Tests must not be weakened to pass a batch.
- AI and direct-agent write attempts must not append formal state events.
- Fact provenance and visibility labels must survive append and replay.

## B010 Application

- CODEX-0307 contributes no business Rust output in this batch.
- B010 current-safe tests add module-specific no-leak replay assertions and no-event-on-deny assertions.
- Stage-level fixture coverage remains in `crates/trpg-domain-core/tests/s02_fixture_acceptance_contract_tests.rs`.
