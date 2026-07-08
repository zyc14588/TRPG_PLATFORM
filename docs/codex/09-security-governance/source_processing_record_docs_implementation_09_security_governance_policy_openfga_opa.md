# Source Processing Record: Policy OpenFGA OPA

Batch: `BATCH-036-09-security-governance`
Prompt ID: `CODEX-0821-09-SECURITY-GOVERNANCE-a664f74925`
Prompt file: `codex-prompts/09-security-governance/P0041.md`
Current module: `security_governance::source_processing_record_docs_implementation_09_security_governance_policy_openfga_opa`

## Disposition

Documentation/traceability only. This record tracks OpenFGA/OPA provenance and does not create business code, migrations, API handlers, NATS subjects, event schemas, metrics, workflows, or tests.

## Current-Safe Notes

- Current policy behavior is implemented by `security_governance::policy_openfga_opa`.
- Relationship authorization and context policy are conjunctive gates.
- Deny, error, or indeterminate policy results must fail closed.

## Validation Responsibility

- Verify the prompt row maps to this Markdown file.
- Verify Rust and OPA tests cover fail-closed policy combinations.
- Verify no historical source path becomes executable naming.

