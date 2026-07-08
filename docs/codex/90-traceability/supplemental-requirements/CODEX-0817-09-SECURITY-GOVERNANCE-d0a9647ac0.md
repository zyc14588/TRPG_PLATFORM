# Supplemental Requirement: CODEX-0817-09-SECURITY-GOVERNANCE-d0a9647ac0

Batch: `BATCH-036-09-security-governance`
Prompt file: `codex-prompts/09-security-governance/P0032.md`
Primary prompt: `CODEX-0800-09-SECURITY-GOVERNANCE-c3d25aee21`
Current module: `security_governance::policy_authz`

## Boundary

This prompt is supplemental only. It does not own Rust source, tests, migrations, API handlers, event schemas, NATS subjects, metrics, or workflow names. Any code effect must be merged into the primary prompt listed above.

## Merge Instructions

- Keep permission decisions current-safe and independent of historical source path names.
- Deny platform roles from changing formal adjudications.
- Permit safety and moderation controls only when they do not directly alter game outcomes.

## Test Responsibility

- Primary tests must cover permission matrix allow/deny rows.
- Primary tests must cover game adjudication override denial.
- Existing S04 tests bind permission matrix fixture rows to Rust assertions.

