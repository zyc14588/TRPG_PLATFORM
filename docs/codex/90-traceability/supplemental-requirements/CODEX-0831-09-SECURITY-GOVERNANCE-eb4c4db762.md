# Supplemental Requirement: CODEX-0831-09-SECURITY-GOVERNANCE-eb4c4db762

Batch: `BATCH-036-09-security-governance`
Prompt file: `codex-prompts/09-security-governance/P0052.md`
Primary prompt: `CODEX-0794-09-SECURITY-GOVERNANCE-3e40b87611`
Current module: `security_governance::audit_log_contract`

## Boundary

This prompt is supplemental only. It does not own Rust source, tests, migrations, API handlers, event schemas, NATS subjects, metrics, or workflow names. Any code effect must be merged into the primary prompt listed above.

## Merge Instructions

- Audit entries must carry actor, correlation, causation, visibility, fact provenance, and policy outcome metadata.
- Public replay must not reveal keeper-only or system-only audit content.
- Audit records must be append-only and not used to mutate Authority Contract.

## Test Responsibility

- Primary tests must cover audit metadata persistence and redacted replay.
- Existing S04 tests cover audit metadata and visibility-filtered replay.

