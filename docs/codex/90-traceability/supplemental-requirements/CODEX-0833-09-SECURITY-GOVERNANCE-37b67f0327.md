# Supplemental Requirement: CODEX-0833-09-SECURITY-GOVERNANCE-37b67f0327

Batch: `BATCH-036-09-security-governance`
Prompt file: `codex-prompts/09-security-governance/P0050.md`
Primary prompt: `CODEX-0083-09-SECURITY-GOVERNANCE-d2a603dc5d`
Current module: `security_governance::data_retention_deletion`

## Boundary

This prompt is supplemental only. It does not own Rust source, tests, migrations, API handlers, event schemas, NATS subjects, metrics, or workflow names. Any code effect must be merged into the primary prompt listed above.

## Merge Instructions

- Deny deletion while legal hold or audit-retention constraints apply.
- Ensure deletion workflows preserve audit facts required for security review.
- Do not use deletion to rewrite GameEvent history or Authority Contract.

## Test Responsibility

- Primary tests must cover legal-hold deletion denial.
- Existing S04 tests cover legal-hold denial with no event append.

