# Supplemental Requirement: CODEX-0812-09-SECURITY-GOVERNANCE-65a556b47d

Batch: `BATCH-036-09-security-governance`
Prompt file: `codex-prompts/09-security-governance/P0028.md`
Primary prompt: `CODEX-0805-09-SECURITY-GOVERNANCE-d25ddec831`
Current module: `security_governance::permission_matrix`

## Boundary

This prompt is supplemental only. It does not own Rust source, tests, migrations, API handlers, event schemas, NATS subjects, metrics, or workflow names. Any code effect must be merged into the primary prompt listed above.

## Merge Instructions

- Keep platform administration separate from game adjudication authority.
- Allow moderation and room safety actions without permitting dice override, history rewrite, or Authority Contract mutation.
- Make permission decisions mode-aware for HUMAN_KP and AI_KP campaigns.

## Test Responsibility

- Primary tests must cover allow/deny rows from `fixtures/security/permission_matrix.v1.json.md`.
- Primary tests must include negative cases for server owner and moderator attempts to alter game rulings.
- Existing S04 tests bind the permission matrix fixture to Rust assertions.

