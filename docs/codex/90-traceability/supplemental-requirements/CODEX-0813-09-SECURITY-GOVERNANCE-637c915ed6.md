# Supplemental Requirement: CODEX-0813-09-SECURITY-GOVERNANCE-637c915ed6

Batch: `BATCH-036-09-security-governance`
Prompt file: `codex-prompts/09-security-governance/P0029.md`
Primary prompt: `CODEX-0085-09-SECURITY-GOVERNANCE-4517fccc2d`
Current module: `security_governance::policy_openfga_opa`

## Boundary

This prompt is supplemental only. It does not own Rust source, tests, migrations, API handlers, event schemas, NATS subjects, metrics, or workflow names. Any code effect must be merged into the primary prompt listed above.

## Merge Instructions

- Treat OpenFGA relationship checks and OPA context checks as conjunctive gates.
- Fail closed if either gate denies, errors, or returns an indeterminate decision.
- Preserve audit metadata for every permit and deny decision.

## Test Responsibility

- Primary tests must cover OpenFGA permit plus OPA deny and OpenFGA deny plus OPA permit.
- `opa test policy/opa` is the stage-level policy check when OPA is available.
- Existing S04 Rust and OPA tests provide current executable coverage.

