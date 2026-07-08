# Supplemental Requirement: CODEX-0816-09-SECURITY-GOVERNANCE-f3a5e6e8b2

Batch: `BATCH-036-09-security-governance`
Prompt file: `codex-prompts/09-security-governance/P0033.md`
Primary prompt: `CODEX-0798-09-SECURITY-GOVERNANCE-c77d457529`
Current module: `security_governance::security_privacy_copyright`

## Boundary

This prompt is supplemental only. It does not own Rust source, tests, migrations, API handlers, event schemas, NATS subjects, metrics, or workflow names. Any code effect must be merged into the primary prompt listed above.

## Merge Instructions

- Enforce production provider security boundaries before any model route is accepted.
- Reject unauthenticated exposed local providers and placeholder API keys in production.
- Preserve explicit audit for any model boundary denial.

## Test Responsibility

- Primary tests must cover production placeholder provider rejection.
- Primary tests must cover no silent cross-boundary fallback.
- Existing S04 tests cover production local provider rejection.

