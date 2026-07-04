# Source Processing Record - Visibility Enforcement Points

Batch: B010  
Prompt ID: CODEX-0306-02-DOMAIN-CORE-d5a05a9dda  
Role: documentation-or-traceability  
Current-safe output: docs/codex/02-domain-core/source_processing_record_docs_implementation_09_security_governance_visibility_enforcement_points.md

## Boundary

- This record is Markdown-only traceability maintenance.
- It does not own Rust src/test output, migrations, API handlers, NATS subjects, metric labels, workflows, or event schemas.
- Source path, source hash, and older version markers in the prompt remain provenance only.

## Normalized Anchors

- Visibility labels are carried on `CommandEnvelope` and copied to `EventEnvelope`.
- Replay must filter through `EventStore::replay_visible` or an equivalent policy-aware read path.
- Keeper-only and system/private labels must not appear in player replay/export surfaces.
- Fact provenance must travel with event records so derived outputs can be audited and rebuilt.

## B010 Application

- CODEX-0306 contributes no business Rust output in this batch.
- B010 current-safe tests add replay assertions for player redaction across authority, rule runtime, investigation, character/combat/SAN/chase, and projection modules.
- No OpenFGA, OPA, API, SQLx, or NATS implementation was added from this documentation-only prompt.
