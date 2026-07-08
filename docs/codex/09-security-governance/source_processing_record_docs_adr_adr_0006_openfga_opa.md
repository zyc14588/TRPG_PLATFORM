# Source Processing Record: ADR-0006 OpenFGA OPA

Batch: `BATCH-036-09-security-governance`
Prompt ID: `CODEX-0818-09-SECURITY-GOVERNANCE-72288ee9c4`
Prompt file: `codex-prompts/09-security-governance/P0034.md`
Current module: `security_governance::source_processing_record_docs_adr_adr_0006_openfga_opa`

## Disposition

Documentation/traceability only. The source-processing record supports current OpenFGA plus OPA policy governance, but it does not create Rust modules, migrations, handlers, NATS subjects, event schemas, metrics, workflows, or tests.

## Current-Safe Notes

- Historical version labels and source hashes are provenance only.
- Current executable policy behavior belongs to `security_governance::adr_0006_openfga_opa` and the existing S04 policy tests.
- Authority Contract, visibility propagation, fact provenance, Event Store canon, and Policy Gate default-deny remain mandatory.

## Validation Responsibility

- Verify the prompt row maps to this Markdown file.
- Verify no historical path token becomes an executable artifact name.
- Verify S04 checks cover the current policy implementation.

