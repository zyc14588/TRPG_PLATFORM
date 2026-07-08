# Supplemental Requirement: CODEX-0815-09-SECURITY-GOVERNANCE-63b6032c87

Batch: `BATCH-036-09-security-governance`
Prompt file: `codex-prompts/09-security-governance/P0031.md`
Primary prompt: `CODEX-0086-09-SECURITY-GOVERNANCE-bb407cb7fc`
Current module: `security_governance::security_privacy`

## Boundary

This prompt is supplemental only. It does not own Rust source, tests, migrations, API handlers, event schemas, NATS subjects, metrics, or workflow names. Any code effect must be merged into the primary prompt listed above.

## Merge Instructions

- Reject direct Agent write paths for formal game state.
- Apply privacy checks before summaries, exports, replay, RAG indexing, logs, and metrics.
- Keep policy denial records audit-safe and visibility-preserving.

## Test Responsibility

- Primary tests must include direct Agent write denial.
- Primary tests must include restricted visibility negative cases.
- Existing S04 tests cover direct Agent write denial and visibility redaction.

