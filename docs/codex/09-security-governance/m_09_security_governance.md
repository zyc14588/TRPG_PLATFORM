# Security Governance Module Map

Batch: `BATCH-035-09-security-governance`

This document is the current-safe traceability map for the security governance slice. It is bounded by `docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md`, `CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`, and the normalized prompt/output maps.

## Current-Safe Outputs

The implementation crate for this batch is `crates/trpg-security-governance`. The crate exposes policy checks as domain contracts only; it does not call OpenAI, Ollama, llama.cpp, or any bare LLM provider. Formal governance decisions are represented as commands evaluated into event-store records.

| Prompt IDs | Current-safe output |
|---|---|
| CODEX-0083, CODEX-0796, CODEX-0803 | `security_governance::data_retention_deletion` |
| CODEX-0085, CODEX-0797, CODEX-0809 | `security_governance::policy_openfga_opa` |
| CODEX-0086 | `security_governance::security_privacy` |
| CODEX-0087 | `security_governance::visibility_enforcement_points` |
| CODEX-0793 | `security_governance::adr_0006_openfga_opa` |
| CODEX-0794, CODEX-0806 | `security_governance::audit_log_contract` |
| CODEX-0795, CODEX-0808 | `security_governance::copyright_boundary` |
| CODEX-0798, CODEX-0799, CODEX-0807 | `security_governance::security_privacy_copyright` |
| CODEX-0800 | `security_governance::policy_authz` |
| CODEX-0801 | `security_governance::policy_authorization` |
| CODEX-0802 | `security_governance::privacy_copyright` |
| CODEX-0804 | `security_governance::readme` |
| CODEX-0805 | `security_governance::permission_matrix` |

## Invariants

- OpenFGA and OPA decisions are fail-closed: any deny blocks the command before event append.
- Agent, provider, or direct business writes cannot create formal state; shared-kernel command envelope validation remains the write boundary.
- Visibility checks preserve source labels, redact or omit restricted derived output, and produce stable error codes for S04 fixtures.
- Local providers are allowed in development fixtures, but production exposure without authentication or placeholder API keys is denied.
- Copyrighted commercial full-text import/export is denied; short quotation remains allowed as a constrained boundary case.
