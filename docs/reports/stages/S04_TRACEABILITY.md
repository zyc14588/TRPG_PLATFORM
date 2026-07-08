# S04 Traceability

Stage: `S04-security-governance-policy`
Date: `2026-07-09`
Result: `PASS`

## Batch Coverage Summary

| Batch | Rows | Primary | Supplemental | Traceability/docs | Result |
|---|---:|---:|---:|---:|---:|
| `BATCH-035-09-security-governance` | 25 | 12 | 8 | 5 | PASS |
| `BATCH-036-09-security-governance` | 25 | 0 | 13 | 12 | PASS |

## BATCH-035 Prompt Coverage

| Prompt ID | Role | Evidence | Test / fixture | Result |
|---|---|---|---|---:|
| `CODEX-0080-09-SECURITY-GOVERNANCE-48409d1e3e` | documentation-or-traceability | `docs/codex/09-security-governance/m_09_security_governance.md` | Evidence review | PASS |
| `CODEX-0081-09-SECURITY-GOVERNANCE-cbd77d8f5a` | documentation-or-traceability | `docs/codex/09-security-governance/audit_log_contract.md` | `audit_log_contract_persists_audit_metadata` | PASS |
| `CODEX-0082-09-SECURITY-GOVERNANCE-462eb947b4` | documentation-or-traceability | `docs/codex/09-security-governance/copyright_boundary.md` | `copyright_boundary_rejects_commercial_full_text` | PASS |
| `CODEX-0083-09-SECURITY-GOVERNANCE-d2a603dc5d` | primary-implementation | `crates/trpg-security-governance/src/data_retention_deletion.rs` | `data_retention_deletion_rejects_legal_hold` | PASS |
| `CODEX-0084-09-SECURITY-GOVERNANCE-3819eb6712` | documentation-or-traceability | `docs/codex/09-security-governance/permission_matrix.md` | `policy_authz_matches_permission_matrix_fixture`; `permission_matrix.v1.json.md` | PASS |
| `CODEX-0085-09-SECURITY-GOVERNANCE-4517fccc2d` | primary-implementation | `crates/trpg-security-governance/src/policy_openfga_opa.rs`; `policy/opa/security_governance.rego`; `policy/openfga/security_governance.fga` | `policy_openfga_opa_fails_closed`; `openfga_security_governance_model_matches_permission_matrix_fixture`; `opa test policy/opa` | PASS |
| `CODEX-0086-09-SECURITY-GOVERNANCE-bb407cb7fc` | primary-implementation | `crates/trpg-security-governance/src/security_privacy.rs` | `security_privacy_rejects_direct_agent_write_path` | PASS |
| `CODEX-0087-09-SECURITY-GOVERNANCE-85f31a0ce6` | primary-implementation | `crates/trpg-security-governance/src/visibility_enforcement_points.rs` | `visibility_enforcement_points_redacts_stage_cases`; S04 visibility fixtures | PASS |
| `CODEX-0793-09-SECURITY-GOVERNANCE-b3f02c351f` | primary-implementation | `crates/trpg-security-governance/src/adr_0006_openfga_opa.rs`; `policy/opa/security_governance.rego` | `adr_0006_openfga_opa_uses_current_safe_names`; `opa test policy/opa` | PASS |
| `CODEX-0794-09-SECURITY-GOVERNANCE-3e40b87611` | primary-implementation | `crates/trpg-security-governance/src/audit_log_contract.rs` | `audit_log_contract_persists_audit_metadata` | PASS |
| `CODEX-0795-09-SECURITY-GOVERNANCE-33b7bf7fe8` | primary-implementation | `crates/trpg-security-governance/src/copyright_boundary.rs` | `copyright_boundary_rejects_commercial_full_text` | PASS |
| `CODEX-0796-09-SECURITY-GOVERNANCE-fbf59a76b6` | supplemental-requirement | Merged into `CODEX-0083` | No separate Rust output; covered by `data_retention_deletion_rejects_legal_hold` | PASS |
| `CODEX-0797-09-SECURITY-GOVERNANCE-d2685460c3` | supplemental-requirement | Merged into `CODEX-0085` | No separate Rust output; covered by OpenFGA/OPA tests | PASS |
| `CODEX-0798-09-SECURITY-GOVERNANCE-c77d457529` | primary-implementation | `crates/trpg-security-governance/src/security_privacy_copyright.rs` | `security_privacy_copyright_denies_prod_placeholder_provider` | PASS |
| `CODEX-0799-09-SECURITY-GOVERNANCE-5f28723170` | supplemental-requirement | Merged into `CODEX-0798` | No separate Rust output; covered by provider boundary tests | PASS |
| `CODEX-0800-09-SECURITY-GOVERNANCE-c3d25aee21` | primary-implementation | `crates/trpg-security-governance/src/policy_authz.rs` | `policy_authz_matches_permission_matrix_fixture` | PASS |
| `CODEX-0801-09-SECURITY-GOVERNANCE-939f88b104` | primary-implementation | `crates/trpg-security-governance/src/policy_authorization.rs` | `policy_authorization_enforces_authority_specific_actions` | PASS |
| `CODEX-0802-09-SECURITY-GOVERNANCE-bee99ae20d` | primary-implementation | `crates/trpg-security-governance/src/privacy_copyright.rs` | `privacy_copyright_blocks_ai_internal_export` | PASS |
| `CODEX-0803-09-SECURITY-GOVERNANCE-c393e21259` | supplemental-requirement | Merged into `CODEX-0083` | No separate Rust output; covered by data retention tests | PASS |
| `CODEX-0804-09-SECURITY-GOVERNANCE-f8e9581ea3` | primary-implementation | `crates/trpg-security-governance/src/readme.rs` | `readme_contract_lists_required_governance_metrics` | PASS |
| `CODEX-0805-09-SECURITY-GOVERNANCE-d25ddec831` | primary-implementation | `crates/trpg-security-governance/src/permission_matrix.rs` | `permission_matrix_covers_provider_certification_and_fallback`; `permission_matrix.v1.json.md` | PASS |
| `CODEX-0806-09-SECURITY-GOVERNANCE-71fd68ef9d` | supplemental-requirement | Merged into `CODEX-0794` | No separate Rust output; covered by audit metadata tests | PASS |
| `CODEX-0807-09-SECURITY-GOVERNANCE-59cdbb516f` | supplemental-requirement | Merged into `CODEX-0798` | No separate Rust output; covered by provider boundary tests | PASS |
| `CODEX-0808-09-SECURITY-GOVERNANCE-b61168b2da` | supplemental-requirement | Merged into `CODEX-0795` | No separate Rust output; covered by copyright boundary tests | PASS |
| `CODEX-0809-09-SECURITY-GOVERNANCE-d01d09b6f0` | supplemental-requirement | Merged into `CODEX-0085` | No separate Rust output; covered by OpenFGA/OPA tests | PASS |

## BATCH-036 Prompt Coverage

| Prompt ID | Role | Evidence | Test / fixture | Result |
|---|---|---|---|---:|
| `CODEX-0810-09-SECURITY-GOVERNANCE-d4514a1929` | supplemental-requirement | `docs/codex/90-traceability/supplemental-requirements/CODEX-0810-09-SECURITY-GOVERNANCE-d4514a1929.md` | Merged into `CODEX-0801`; `policy_authorization_enforces_authority_specific_actions` | PASS |
| `CODEX-0811-09-SECURITY-GOVERNANCE-d9cc760654` | supplemental-requirement | `docs/codex/90-traceability/supplemental-requirements/CODEX-0811-09-SECURITY-GOVERNANCE-d9cc760654.md` | Merged into `CODEX-0802`; `privacy_copyright_blocks_ai_internal_export` | PASS |
| `CODEX-0812-09-SECURITY-GOVERNANCE-65a556b47d` | supplemental-requirement | `docs/codex/90-traceability/supplemental-requirements/CODEX-0812-09-SECURITY-GOVERNANCE-65a556b47d.md` | Merged into `CODEX-0805`; `permission_matrix.v1.json.md` | PASS |
| `CODEX-0813-09-SECURITY-GOVERNANCE-637c915ed6` | supplemental-requirement | `docs/codex/90-traceability/supplemental-requirements/CODEX-0813-09-SECURITY-GOVERNANCE-637c915ed6.md` | Merged into `CODEX-0085`; OpenFGA model and OPA tests | PASS |
| `CODEX-0814-09-SECURITY-GOVERNANCE-8063678545` | supplemental-requirement | `docs/codex/90-traceability/supplemental-requirements/CODEX-0814-09-SECURITY-GOVERNANCE-8063678545.md` | Merged into `CODEX-0804`; metrics/readme contract test | PASS |
| `CODEX-0815-09-SECURITY-GOVERNANCE-63b6032c87` | supplemental-requirement | `docs/codex/90-traceability/supplemental-requirements/CODEX-0815-09-SECURITY-GOVERNANCE-63b6032c87.md` | Merged into `CODEX-0086`; direct-agent write denial test | PASS |
| `CODEX-0816-09-SECURITY-GOVERNANCE-f3a5e6e8b2` | supplemental-requirement | `docs/codex/90-traceability/supplemental-requirements/CODEX-0816-09-SECURITY-GOVERNANCE-f3a5e6e8b2.md` | Merged into `CODEX-0798`; provider boundary tests | PASS |
| `CODEX-0817-09-SECURITY-GOVERNANCE-d0a9647ac0` | supplemental-requirement | `docs/codex/90-traceability/supplemental-requirements/CODEX-0817-09-SECURITY-GOVERNANCE-d0a9647ac0.md` | Merged into `CODEX-0800`; permission matrix authz test | PASS |
| `CODEX-0818-09-SECURITY-GOVERNANCE-72288ee9c4` | documentation-or-traceability | `docs/codex/09-security-governance/source_processing_record_docs_adr_adr_0006_openfga_opa.md` | B036 traceability-only row; no Rust output | PASS |
| `CODEX-0819-09-SECURITY-GOVERNANCE-eb42caa011` | documentation-or-traceability | `docs/codex/09-security-governance/source_processing_record_docs_implementation_09_security_governance_audit_log_contract.md` | B036 traceability-only row; no Rust output | PASS |
| `CODEX-0820-09-SECURITY-GOVERNANCE-dbea6b9f4e` | documentation-or-traceability | `docs/codex/09-security-governance/source_processing_record_docs_implementation_09_security_governance_permission_matrix.md` | B036 traceability-only row; no Rust output | PASS |
| `CODEX-0821-09-SECURITY-GOVERNANCE-a664f74925` | documentation-or-traceability | `docs/codex/09-security-governance/source_processing_record_docs_implementation_09_security_governance_policy_openfga_opa.md` | B036 traceability-only row; no Rust output | PASS |
| `CODEX-0822-09-SECURITY-GOVERNANCE-ab4ffcb405` | documentation-or-traceability | `docs/codex/09-security-governance/source_processing_record_docs_implementation_09_security_governance_data_retention_deletion.md` | B036 traceability-only row; no Rust output | PASS |
| `CODEX-0823-09-SECURITY-GOVERNANCE-d8f6bc7914` | documentation-or-traceability | `docs/codex/09-security-governance/source_processing_record_docs_implementation_09_security_governance_security_privacy.md` | B036 traceability-only row; no Rust output | PASS |
| `CODEX-0824-09-SECURITY-GOVERNANCE-8480837db1` | documentation-or-traceability | `docs/codex/09-security-governance/source_processing_record_docs_implementation_09_security_governance_readme.md` | B036 traceability-only row; no Rust output | PASS |
| `CODEX-0825-09-SECURITY-GOVERNANCE-0883adf3e0` | documentation-or-traceability | `docs/codex/09-security-governance/source_processing_record_docs_implementation_09_security_governance_copyright_boundary.md` | B036 traceability-only row; no Rust output | PASS |
| `CODEX-0826-09-SECURITY-GOVERNANCE-3fcfa9e72e` | documentation-or-traceability | `docs/codex/09-security-governance/source_processing_record_docs_implementation_90_traceability_source_breakdown_platform_policy_authz.md` | B036 traceability-only row; no Rust output | PASS |
| `CODEX-0827-09-SECURITY-GOVERNANCE-0dd8b94eca` | documentation-or-traceability | `docs/codex/09-security-governance/source_processing_record_docs_implementation_90_traceability_source_breakdown_platform_security_privacy_copyright.md` | B036 traceability-only row; no Rust output | PASS |
| `CODEX-0828-09-SECURITY-GOVERNANCE-6b9f1ad992` | documentation-or-traceability | `docs/codex/09-security-governance/source_processing_record_docs_platform_policy_authz.md` | B036 traceability-only row; no Rust output | PASS |
| `CODEX-0829-09-SECURITY-GOVERNANCE-561b5440b4` | documentation-or-traceability | `docs/codex/09-security-governance/source_processing_record_docs_platform_security_privacy_copyright.md` | B036 traceability-only row; no Rust output | PASS |
| `CODEX-0830-09-SECURITY-GOVERNANCE-9b64042016` | supplemental-requirement | `docs/codex/90-traceability/supplemental-requirements/CODEX-0830-09-SECURITY-GOVERNANCE-9b64042016.md` | Merged into `CODEX-0793`; current-safe ADR/OpenFGA-OPA tests | PASS |
| `CODEX-0831-09-SECURITY-GOVERNANCE-eb4c4db762` | supplemental-requirement | `docs/codex/90-traceability/supplemental-requirements/CODEX-0831-09-SECURITY-GOVERNANCE-eb4c4db762.md` | Merged into `CODEX-0794`; audit metadata test | PASS |
| `CODEX-0832-09-SECURITY-GOVERNANCE-ec7c566187` | supplemental-requirement | `docs/codex/90-traceability/supplemental-requirements/CODEX-0832-09-SECURITY-GOVERNANCE-ec7c566187.md` | Merged into `CODEX-0795`; copyright boundary test | PASS |
| `CODEX-0833-09-SECURITY-GOVERNANCE-37b67f0327` | supplemental-requirement | `docs/codex/90-traceability/supplemental-requirements/CODEX-0833-09-SECURITY-GOVERNANCE-37b67f0327.md` | Merged into `CODEX-0083`; data retention test | PASS |
| `CODEX-0834-09-SECURITY-GOVERNANCE-0091b85eae` | supplemental-requirement | `docs/codex/90-traceability/supplemental-requirements/CODEX-0834-09-SECURITY-GOVERNANCE-0091b85eae.md` | Merged into `CODEX-0805`; permission matrix fixture tests | PASS |

## Boundary Statement

- BATCH-036 supplemental rows remain supplemental-only and did not create or modify Rust, migration, API handler, workflow, NATS subject, or executable event-schema outputs.
- The only post-acceptance implementation repair is the CODEX-0085 primary-owned OpenFGA model plus the narrow contract test proving S04 permission fixture alignment.
