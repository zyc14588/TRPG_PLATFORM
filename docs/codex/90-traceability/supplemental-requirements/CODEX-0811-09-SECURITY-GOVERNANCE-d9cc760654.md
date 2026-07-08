# Supplemental Requirement: CODEX-0811-09-SECURITY-GOVERNANCE-d9cc760654

Batch: `BATCH-036-09-security-governance`
Prompt file: `codex-prompts/09-security-governance/P0026.md`
Primary prompt: `CODEX-0802-09-SECURITY-GOVERNANCE-bee99ae20d`
Current module: `security_governance::privacy_copyright`

## Boundary

This prompt is supplemental only. It does not own Rust source, tests, migrations, API handlers, event schemas, NATS subjects, metrics, or workflow names. Any code effect must be merged into the primary prompt listed above.

## Merge Instructions

- Block AI-internal material from player export, public replay, RAG chunks, summaries, and debug logs.
- Enforce copyright-use categories before ingesting or exporting scenario/rules content.
- Keep deletion, retention, privacy, and copyright decisions audit-visible without revealing restricted content.

## Test Responsibility

- Primary tests must include AI-internal export denial.
- Primary tests must include copyright boundary denial for full-text commercial content.
- Existing S04 tests provide executable coverage for AI-internal export and copyright denial.

