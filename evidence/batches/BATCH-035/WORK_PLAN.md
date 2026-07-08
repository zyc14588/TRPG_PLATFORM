# BATCH-035 Work Plan

Batch: `BATCH-035-09-security-governance`

Authority note: the user-provided batch facts say `recognized primary prompt count: 0`, but `batches/B035.md`, `docs/codex/09-security-governance/per-file-prompt-manifest.md`, and the normalized current-safe maps identify 13 primary-implementation prompts, 8 supplemental prompts, and 4 documentation/traceability prompts. This execution follows the repository authority maps and records the conflict here.

## Prompt Mapping

| Prompt ID | Target file or module | Allowed scope | Test responsibility |
|---|---|---|---|
| CODEX-0080 / P0006 | `docs/codex/09-security-governance/m_09_security_governance.md` | Documentation/traceability only | Covered by evidence review |
| CODEX-0081 / P0001 | `docs/codex/09-security-governance/audit_log_contract.md` | Documentation/traceability only | Audit metadata contract test |
| CODEX-0082 / P0002 | `docs/codex/09-security-governance/copyright_boundary.md` | Documentation/traceability only | Copyright boundary contract test |
| CODEX-0083 / P0003 | `crates/trpg-security-governance/src/data_retention_deletion.rs` | Primary implementation | Legal-hold deletion denial |
| CODEX-0084 / P0004 | `docs/codex/09-security-governance/permission_matrix.md` | Documentation/traceability only | Permission matrix contract test |
| CODEX-0085 / P0005 | `crates/trpg-security-governance/src/policy_openfga_opa.rs` | Primary implementation | OpenFGA/OPA fail-closed test |
| CODEX-0086 / P0007 | `crates/trpg-security-governance/src/security_privacy.rs` | Primary implementation | Direct agent write denial |
| CODEX-0087 / P0008 | `crates/trpg-security-governance/src/visibility_enforcement_points.rs` | Primary implementation | Visibility redaction fixture cases |
| CODEX-0793 / P0009 | `crates/trpg-security-governance/src/adr_0006_openfga_opa.rs` | Primary implementation | Current-safe naming and event contract |
| CODEX-0794 / P0010 | `crates/trpg-security-governance/src/audit_log_contract.rs` | Primary implementation | Event metadata/replay visibility |
| CODEX-0795 / P0011 | `crates/trpg-security-governance/src/copyright_boundary.rs` | Primary implementation | Commercial full-text denial |
| CODEX-0796 / P0012 | supplement for CODEX-0083 | Supplemental requirements only | Merged into data retention test |
| CODEX-0797 / P0016 | supplement for CODEX-0085 | Supplemental requirements only | Merged into OpenFGA/OPA test |
| CODEX-0798 / P0015 | `crates/trpg-security-governance/src/security_privacy_copyright.rs` | Primary implementation | Provider/copyright/privacy gate test |
| CODEX-0799 / P0013 | supplement for CODEX-0798 | Supplemental requirements only | Merged into provider boundary test |
| CODEX-0800 / P0014 | `crates/trpg-security-governance/src/policy_authz.rs` | Primary implementation | Permission matrix fixture test |
| CODEX-0801 / P0017 | `crates/trpg-security-governance/src/policy_authorization.rs` | Primary implementation | Authority-specific permission test |
| CODEX-0802 / P0018 | `crates/trpg-security-governance/src/privacy_copyright.rs` | Primary implementation | AI-internal export denial |
| CODEX-0803 / P0022 | supplement for CODEX-0083 | Supplemental requirements only | Merged into data retention test |
| CODEX-0804 / P0027 | `crates/trpg-security-governance/src/readme.rs` | Primary implementation | Required metric/event contract |
| CODEX-0805 / P0024 | `crates/trpg-security-governance/src/permission_matrix.rs` | Primary implementation | Permission/model/fallback fixture test |
| CODEX-0806 / P0020 | supplement for CODEX-0794 | Supplemental requirements only | Merged into audit metadata test |
| CODEX-0807 / P0019 | supplement for CODEX-0798 | Supplemental requirements only | Merged into provider boundary test |
| CODEX-0808 / P0021 | supplement for CODEX-0795 | Supplemental requirements only | Merged into copyright boundary test |
| CODEX-0809 / P0023 | supplement for CODEX-0085 | Supplemental requirements only | Merged into OpenFGA/OPA test |

## Execution Slice

1. Add a minimal `trpg-security-governance` workspace crate.
2. Implement current-safe module entry points and shared governance contracts.
3. Add one focused B035 integration test suite with prompt-named scenarios.
4. Add documentation/traceability Markdown for the four documentation prompts.
5. Run minimal crate checks first, then S04-related checks that are available in this workspace.
