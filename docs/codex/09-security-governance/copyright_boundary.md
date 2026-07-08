# Copyright Boundary

Batch: `BATCH-035-09-security-governance`

The copyright boundary is a policy contract for import, export, summary, and private-reference flows. It is not a content ingestion pipeline and does not grant provider access.

## Rules

- Original and permissively licensed material can be handled by normal visibility and provenance gates.
- Commercial copyrighted full-text import and player export are denied.
- Short quotation is allowed only as a narrow boundary case and must still carry visibility and fact provenance.
- AI-generated summaries cannot remove visibility labels or provenance from their source facts.

## Covered By

- `crates/trpg-security-governance/src/copyright_boundary.rs`
- `crates/trpg-security-governance/src/privacy_copyright.rs`
- `crates/trpg-security-governance/src/security_privacy_copyright.rs`
- `crates/trpg-security-governance/tests/batch_035_security_governance_contract_tests.rs`
