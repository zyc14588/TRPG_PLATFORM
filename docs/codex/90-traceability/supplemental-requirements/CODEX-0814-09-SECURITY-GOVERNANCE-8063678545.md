# Supplemental Requirement: CODEX-0814-09-SECURITY-GOVERNANCE-8063678545

Batch: `BATCH-036-09-security-governance`
Prompt file: `codex-prompts/09-security-governance/P0030.md`
Primary prompt: `CODEX-0804-09-SECURITY-GOVERNANCE-f8e9581ea3`
Current module: `security_governance::readme`

## Boundary

This prompt is supplemental only. It does not own Rust source, tests, migrations, API handlers, event schemas, NATS subjects, metrics, or workflow names. Any code effect must be merged into the primary prompt listed above.

## Merge Instructions

- Keep the module-level README aligned with OpenFGA, OPA, audit, privacy, copyright, retention, and visibility enforcement responsibilities.
- Document hard gates as mandatory behavior, not logging-only recommendations.
- Ensure metrics and evidence references do not expose restricted facts.

## Test Responsibility

- Primary tests must assert the required governance metric names remain present.
- Existing S04 tests cover the module contract list and event recording path.

