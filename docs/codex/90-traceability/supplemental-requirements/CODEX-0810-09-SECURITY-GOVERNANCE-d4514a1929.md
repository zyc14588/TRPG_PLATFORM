# Supplemental Requirement: CODEX-0810-09-SECURITY-GOVERNANCE-d4514a1929

Batch: `BATCH-036-09-security-governance`
Prompt file: `codex-prompts/09-security-governance/P0025.md`
Primary prompt: `CODEX-0801-09-SECURITY-GOVERNANCE-939f88b104`
Current module: `security_governance::policy_authorization`

## Boundary

This prompt is supplemental only. It does not own Rust source, tests, migrations, API handlers, event schemas, NATS subjects, metrics, or workflow names. Any code effect must be merged into the primary prompt listed above.

## Merge Instructions

- Keep Policy Gate default-deny and require both relationship authorization and context policy approval.
- Reject any authority-mode mismatch before recording a formal decision.
- Preserve visibility labels and fact provenance on every policy decision, audit entry, replay, export, and projection path.
- Record denial reasons without exposing keeper-only, private, AI-internal, or system-only facts.

## Test Responsibility

- Primary tests must cover authority violation denial with no event append.
- Primary tests must cover visibility/provenance replay from policy decisions.
- Existing S04 tests in `crates/trpg-security-governance/tests/batch_035_security_governance_contract_tests.rs` provide the current executable coverage.

