# Audit Log Contract

Batch: `BATCH-035-09-security-governance`

Audit records for this slice are append-only event-store records emitted through `security_governance.decision_recorded`. The event envelope must retain the original command idempotency key, visibility label, fact provenance, correlation id, causation id, authority mode, and authority contract version.

## Required Behavior

- A policy decision is recorded only after command envelope validation succeeds.
- OpenFGA or OPA denial leaves the repository unchanged.
- Visibility is not widened during replay; public replay cannot see keeper-only records, while system replay can.
- Audit records describe the current-safe module name and action, not source archive paths or historic version tokens.

## Covered By

- `crates/trpg-security-governance/src/audit_log_contract.rs`
- `crates/trpg-security-governance/tests/batch_035_security_governance_contract_tests.rs`
