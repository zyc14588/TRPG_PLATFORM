# Supplemental Requirement: CODEX-0832-09-SECURITY-GOVERNANCE-ec7c566187

Batch: `BATCH-036-09-security-governance`
Prompt file: `codex-prompts/09-security-governance/P0053.md`
Primary prompt: `CODEX-0795-09-SECURITY-GOVERNANCE-33b7bf7fe8`
Current module: `security_governance::copyright_boundary`

## Boundary

This prompt is supplemental only. It does not own Rust source, tests, migrations, API handlers, event schemas, NATS subjects, metrics, or workflow names. Any code effect must be merged into the primary prompt listed above.

## Merge Instructions

- Do not bundle unauthorized commercial rules or scenario full text.
- Permit only allowed use categories such as short quote, user-owned upload, or authorized import.
- Keep copyright decisions auditable without copying protected content into logs or evidence.

## Test Responsibility

- Primary tests must cover commercial full-text denial and short-quote allowance.
- Existing S04 tests cover copyright boundary behavior.

