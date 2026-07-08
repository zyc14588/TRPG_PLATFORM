# Supplemental Requirement: CODEX-0830-09-SECURITY-GOVERNANCE-9b64042016

Batch: `BATCH-036-09-security-governance`
Prompt file: `codex-prompts/09-security-governance/P0046.md`
Primary prompt: `CODEX-0793-09-SECURITY-GOVERNANCE-b3f02c351f`
Current module: `security_governance::adr_0006_openfga_opa`

## Boundary

This prompt is supplemental only. It does not own Rust source, tests, migrations, API handlers, event schemas, NATS subjects, metrics, or workflow names. Any code effect must be merged into the primary prompt listed above.

## Merge Instructions

- Keep ADR-0006 behavior as current guidance for combining OpenFGA relationship checks with OPA context checks.
- Do not let ADR provenance introduce historical source-path names into executable outputs.
- Verify the current-safe policy decision event name remains stable.

## Test Responsibility

- Primary tests must check current-safe module and event naming.
- Existing S04 tests cover current-safe ADR module and event constants.

