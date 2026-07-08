# BATCH-033 Work Plan

Batch: BATCH-033-08-platform-infrastructure -- Strict Governance Final
Date: 2026-07-08

## Scope Resolution

- Declared prompt count: 25.
- User/start-prompt supplied primary count: 0.
- Canonical `batches/B033.md` plus per-file prompts identify 3 primary implementation prompts:
  - CODEX-0786 / P0071 -> `crates/trpg-platform/src/api_contracts.rs`.
  - CODEX-0789 / P0074 -> `crates/trpg-platform/src/plugin_sdk.rs`.
  - CODEX-0790 / P0075 -> `crates/trpg-platform/src/policy_authz.rs`.
- Execution follows the normalized current-safe mappings after reading the required governance indexes. Historical V3/V4/V5/V6 names remain provenance only.

## Prompt Map

| Prompt ID | Per-file prompt | Role | Current-safe target | Allowed change scope | Test responsibility |
|---|---:|---|---|---|---|
| CODEX-0766 | P0049 | traceability-maintenance | `docs/codex/08-platform-infrastructure/source_processing_record_docs_implementation_90_traceability_source_breakdown_platform_reliability_performance.md` | Markdown traceability only; no Rust output. | Covered by prompt traceability evidence. |
| CODEX-0767 | P0052 | traceability-maintenance | `docs/codex/08-platform-infrastructure/source_breakdown_platform_deployment_ops.md` | Markdown traceability only; no Rust output. | Covered by prompt traceability evidence. |
| CODEX-0768 | P0048 | traceability-maintenance | `docs/codex/08-platform-infrastructure/source_breakdown_platform_observability.md` | Markdown traceability only; no Rust output. | Covered by prompt traceability evidence. |
| CODEX-0769 | P0056 | traceability-maintenance | `docs/codex/08-platform-infrastructure/source_processing_record_docs_platform_deployment_ops.md` | Markdown traceability only; no Rust output. | Covered by prompt traceability evidence. |
| CODEX-0770 | P0047 | traceability-maintenance | `docs/codex/08-platform-infrastructure/source_processing_record_docs_platform_observability.md` | Markdown traceability only; no Rust output. | Covered by prompt traceability evidence. |
| CODEX-0771 | P0055 | traceability-maintenance | `docs/codex/08-platform-infrastructure/source_processing_record_docs_platform_reliability_performance.md` | Markdown traceability only; no Rust output. | Covered by prompt traceability evidence. |
| CODEX-0772 | P0060 | supplemental-requirement | Supplemental for CODEX-0074 background workers | Supplemental constraints only; no implementation edits. | Covered by prompt traceability evidence. |
| CODEX-0773 | P0057 | supplemental-requirement | Supplemental for CODEX-0075 deployment ops | Supplemental constraints only; no implementation edits. | Covered by prompt traceability evidence. |
| CODEX-0774 | P0061 | supplemental-requirement | Supplemental for CODEX-0076 local dev environment | Supplemental constraints only; no implementation edits. | Covered by prompt traceability evidence. |
| CODEX-0775 | P0063 | supplemental-requirement | Supplemental for CODEX-0077 object storage | Supplemental constraints only; no implementation edits. | Covered by prompt traceability evidence. |
| CODEX-0776 | P0059 | supplemental-requirement | Supplemental for CODEX-0078 observability | Supplemental constraints only; no implementation edits. | Covered by prompt traceability evidence. |
| CODEX-0777 | P0062 | supplemental-requirement | Supplemental for CODEX-0079 performance budget | Supplemental constraints only; no implementation edits. | Covered by prompt traceability evidence. |
| CODEX-0778 | P0058 | supplemental-requirement | Supplemental for CODEX-0738 platform README | Supplemental constraints only; no implementation edits. | Covered by prompt traceability evidence. |
| CODEX-0779 | P0066 | supplemental-requirement | Supplemental for CODEX-0752 API contracts impl | Supplemental constraints only; no implementation edits. | Covered by prompt traceability evidence. |
| CODEX-0780 | P0065 | supplemental-requirement | Supplemental for CODEX-0753 deployment ops impl | Supplemental constraints only; no implementation edits. | Covered by prompt traceability evidence. |
| CODEX-0781 | P0068 | supplemental-requirement | Supplemental for CODEX-0754 observability impl | Supplemental constraints only; no implementation edits. | Covered by prompt traceability evidence. |
| CODEX-0782 | P0069 | supplemental-requirement | Supplemental for CODEX-0755 plugin SDK impl | Supplemental constraints only; no implementation edits. | Covered by prompt traceability evidence. |
| CODEX-0783 | P0067 | supplemental-requirement | Supplemental for CODEX-0756 policy authz impl | Supplemental constraints only; no implementation edits. | Covered by prompt traceability evidence. |
| CODEX-0784 | P0070 | supplemental-requirement | Supplemental for CODEX-0757 reliability/performance impl | Supplemental constraints only; no implementation edits. | Covered by prompt traceability evidence. |
| CODEX-0785 | P0064 | supplemental-requirement | Supplemental for CODEX-0758 security/privacy/copyright impl | Supplemental constraints only; no implementation edits. | Covered by prompt traceability evidence. |
| CODEX-0786 | P0071 | primary-implementation | `crates/trpg-platform/src/api_contracts.rs`; `crates/trpg-platform/tests/api_contracts_contract_tests.rs` | Implement current-safe API contract governance module and focused tests. | Minimal test target plus platform crate stage test. |
| CODEX-0787 | P0072 | supplemental-requirement | Supplemental for CODEX-0075 deployment ops | Supplemental constraints only; no implementation edits. | Covered by prompt traceability evidence. |
| CODEX-0788 | P0073 | supplemental-requirement | Supplemental for CODEX-0078 observability | Supplemental constraints only; no implementation edits. | Covered by prompt traceability evidence. |
| CODEX-0789 | P0074 | primary-implementation | `crates/trpg-platform/src/plugin_sdk.rs`; `crates/trpg-platform/tests/plugin_sdk_contract_tests.rs` | Implement current-safe plugin SDK governance module and focused tests. | Minimal test target plus platform crate stage test. |
| CODEX-0790 | P0075 | primary-implementation | `crates/trpg-platform/src/policy_authz.rs`; `crates/trpg-platform/tests/policy_authz_contract_tests.rs` | Implement current-safe policy authorization governance module and focused tests. | Minimal test target plus platform crate stage test. |

## Test Plan

1. `cargo fmt --all -- --check`
2. `cargo test -p trpg-platform --test api_contracts_contract_tests`
3. `cargo test -p trpg-platform --test plugin_sdk_contract_tests`
4. `cargo test -p trpg-platform --test policy_authz_contract_tests`
5. `cargo test -p trpg-platform`
6. `cargo check`

## Evidence Outputs

- `evidence/batches/BATCH-033/plan.md`
- `evidence/batches/BATCH-033/changed-files.txt`
- `evidence/batches/BATCH-033/test-output.txt`
- `evidence/batches/BATCH-033/prompt-traceability.md`
- `evidence/batches/BATCH-033/handoff.md`
- `evidence/batches/BATCH-033/acceptance-report.md`
- `evidence/batches/BATCH-033/acceptance-test-output.txt`
