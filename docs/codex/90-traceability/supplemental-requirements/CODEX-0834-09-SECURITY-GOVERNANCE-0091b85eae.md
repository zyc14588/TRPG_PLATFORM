# Supplemental Requirement: CODEX-0834-09-SECURITY-GOVERNANCE-0091b85eae

Batch: `BATCH-036-09-security-governance`
Prompt file: `codex-prompts/09-security-governance/P0047.md`
Primary prompt: `CODEX-0805-09-SECURITY-GOVERNANCE-d25ddec831`
Current module: `security_governance::permission_matrix`

## Boundary

This prompt is supplemental only. It does not own Rust source, tests, migrations, API handlers, event schemas, NATS subjects, metrics, or workflow names. Any code effect must be merged into the primary prompt listed above.

## Merge Instructions

- Reaffirm the permission matrix rows for platform roles, KP roles, players, moderators, and workflows.
- Deny dice override, AI decision override, history rewrite, and Authority Contract mutation outside fork flow.
- Keep safety pause and moderation separate from formal adjudication.

## Test Responsibility

- Primary tests must cover permission matrix fixture rows and authority-mode-specific actions.
- Existing S04 tests cover fixture-bound permission decisions.

