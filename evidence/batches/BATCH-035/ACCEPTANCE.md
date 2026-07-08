# BATCH-035 Acceptance Evidence

Batch: `BATCH-035-09-security-governance`
Repair date: `2026-07-09`

## Scope Result

- Current-safe mappings were applied before implementation.
- No `source-archive/**` content was used as an executable prompt source.
- No historical V3/V4/V5/V6 path, hash, or legacy source token was introduced into module, event, metric, workflow, or test names.
- The implementation keeps AI/model access outside the business layer. This batch adds policy contracts only and does not call OpenAI, Ollama, llama.cpp, or any bare LLM.
- Formal decisions go through `CommandEnvelope -> EventStore` using shared-kernel validation.
- `policy/opa/security_governance.rego` and `policy/opa/security_governance_test.rego` were added as current-safe OPA policy artifacts.
- S04 visibility and permission fixtures are now bound to Rust assertions in `crates/trpg-security-governance/tests/batch_035_security_governance_contract_tests.rs`.

## Prompt Row Acceptance

| Prompt row | Role | Target evidence | Test or non-code reason | Conclusion |
|---|---|---|---|---|
| CODEX-0080 / P0006 | documentation-or-traceability | `docs/codex/09-security-governance/m_09_security_governance.md` | Documentation row; covered by evidence review | PASS |
| CODEX-0081 / P0001 | documentation-or-traceability | `docs/codex/09-security-governance/audit_log_contract.md` | Documentation row; audit metadata covered by `audit_log_contract_persists_audit_metadata` | PASS |
| CODEX-0082 / P0002 | documentation-or-traceability | `docs/codex/09-security-governance/copyright_boundary.md` | Documentation row; copyright denial covered by `copyright_boundary_rejects_commercial_full_text` | PASS |
| CODEX-0083 / P0003 | primary-implementation | `crates/trpg-security-governance/src/data_retention_deletion.rs` | `data_retention_deletion_rejects_legal_hold` | PASS |
| CODEX-0084 / P0004 | documentation-or-traceability | `docs/codex/09-security-governance/permission_matrix.md` | Documentation row; matrix covered by `policy_authz_matches_permission_matrix_fixture` | PASS |
| CODEX-0085 / P0005 | primary-implementation | `crates/trpg-security-governance/src/policy_openfga_opa.rs`; `policy/opa/security_governance.rego` | Rust fail-closed test passes; `opa test policy/opa` passes 11/11 | PASS |
| CODEX-0086 / P0007 | primary-implementation | `crates/trpg-security-governance/src/security_privacy.rs` | `security_privacy_rejects_direct_agent_write_path` | PASS |
| CODEX-0087 / P0008 | primary-implementation | `crates/trpg-security-governance/src/visibility_enforcement_points.rs` | `visibility_enforcement_points_redacts_stage_cases` covers S04 detailed visibility errors and redaction matrix cases | PASS |
| CODEX-0793 / P0009 | primary-implementation | `crates/trpg-security-governance/src/adr_0006_openfga_opa.rs`; `policy/opa/security_governance.rego` | Current-safe/event Rust test passes; `opa test policy/opa` passes 11/11 | PASS |
| CODEX-0794 / P0010 | primary-implementation | `crates/trpg-security-governance/src/audit_log_contract.rs` | `audit_log_contract_persists_audit_metadata` | PASS |
| CODEX-0795 / P0011 | primary-implementation | `crates/trpg-security-governance/src/copyright_boundary.rs` | `copyright_boundary_rejects_commercial_full_text` | PASS |
| CODEX-0796 / P0012 | supplemental-requirement | Merge target `security_governance::data_retention_deletion` | Supplemental merged into P0003 legal-hold deletion denial; no separate Rust output created | PASS |
| CODEX-0797 / P0016 | supplemental-requirement | Merge target `security_governance::policy_openfga_opa` | Supplemental merged into P0005 OPA/OpenFGA scope; OPA tests pass | PASS |
| CODEX-0798 / P0015 | primary-implementation | `crates/trpg-security-governance/src/security_privacy_copyright.rs` | `security_privacy_copyright_denies_prod_placeholder_provider` | PASS |
| CODEX-0799 / P0013 | supplemental-requirement | Merge target `security_governance::security_privacy_copyright` | Supplemental merged into provider boundary test; no separate Rust output created | PASS |
| CODEX-0800 / P0014 | primary-implementation | `crates/trpg-security-governance/src/policy_authz.rs` | `policy_authz_matches_permission_matrix_fixture` covers all permission matrix rows | PASS |
| CODEX-0801 / P0017 | primary-implementation | `crates/trpg-security-governance/src/policy_authorization.rs` | `policy_authorization_enforces_authority_specific_actions` | PASS |
| CODEX-0802 / P0018 | primary-implementation | `crates/trpg-security-governance/src/privacy_copyright.rs` | `privacy_copyright_blocks_ai_internal_export` | PASS |
| CODEX-0803 / P0022 | supplemental-requirement | Merge target `security_governance::data_retention_deletion` | Supplemental merged into P0003 legal-hold deletion denial; no separate Rust output created | PASS |
| CODEX-0804 / P0027 | primary-implementation | `crates/trpg-security-governance/src/readme.rs` | `readme_contract_lists_required_governance_metrics` | PASS |
| CODEX-0805 / P0024 | primary-implementation | `crates/trpg-security-governance/src/permission_matrix.rs` | `permission_matrix_covers_provider_certification_and_fallback`; permission matrix fixture bound in `policy_authz_matches_permission_matrix_fixture` | PASS |
| CODEX-0806 / P0020 | supplemental-requirement | Merge target `security_governance::audit_log_contract` | Supplemental merged into P0010 audit metadata test; no separate Rust output created | PASS |
| CODEX-0807 / P0019 | supplemental-requirement | Merge target `security_governance::security_privacy_copyright` | Supplemental merged into provider boundary test; no separate Rust output created | PASS |
| CODEX-0808 / P0021 | supplemental-requirement | Merge target `security_governance::copyright_boundary` | Supplemental merged into P0011 copyright boundary test; no separate Rust output created | PASS |
| CODEX-0809 / P0023 | supplemental-requirement | Merge target `security_governance::policy_openfga_opa` | Supplemental merged into P0005 OPA/OpenFGA scope; OPA tests pass | PASS |

## Residual Risks

- This batch implements Rust-level governance contracts plus minimal OPA policy artifacts, not SQL migrations, API handlers, or NATS subjects.
- The user-supplied primary-count fact conflicts with repository maps; this evidence preserves the discrepancy for operator review.
