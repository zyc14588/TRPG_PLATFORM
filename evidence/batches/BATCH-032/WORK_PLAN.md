# BATCH-032 Work Plan

Batch: `BATCH-032-08-platform-infrastructure -- Strict Governance Final`

Execution timestamp: `2026-07-08T10:31:14.8109408+10:00`

Normalized maps applied before execution:

- `docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`
- `docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md`
- `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`

Scope rule: only B032 outputs are implemented. `source-archive/**` remains provenance only. Historical V3/V4/V5/V6/hash/source-path names were not used as current Rust module, event, metric, workflow, migration, test, or output names.

Input discrepancy recorded: the user-provided batch facts said primary prompt count was `0`, but authoritative `batches/B032.md` and current-safe maps list 7 primary implementation prompts (`CODEX-0752` through `CODEX-0758`). Execution followed `batches/B032.md` plus normalized current-safe maps.

| Prompt ID | Role | Current-safe target | Allowed change | Test responsibility |
|---|---|---|---|---|
| CODEX-0741 | supplemental-requirement | `codex-prompts/08-platform-infrastructure/P0028.md` -> `deployment_ops` constraints | No Rust ownership; merge deployment operations constraints into primary/test coverage | `deployment_ops_impl_contract_tests.rs` plus existing deployment tests |
| CODEX-0742 | supplemental-requirement | `codex-prompts/08-platform-infrastructure/P0017.md` -> `observability_audit_trace` constraints | No Rust ownership; preserve audit/trace/redaction requirements | Existing audit trace tests and B032 governance scans |
| CODEX-0743 | supplemental-requirement | `codex-prompts/08-platform-infrastructure/P0021.md` -> `reliability_performance` constraints | No Rust ownership; merge reliability/performance constraints into primary/test coverage | `reliability_performance_impl_contract_tests.rs` |
| CODEX-0744 | supplemental-requirement | `codex-prompts/08-platform-infrastructure/P0029.md` -> `local_dev_environment` constraints | No Rust ownership; local-dev constraints remain B031-owned | Existing local dev tests and B032 governance scans |
| CODEX-0745 | supplemental-requirement | `codex-prompts/08-platform-infrastructure/P0030.md` -> `object_storage` constraints | No Rust ownership; object storage constraints remain B031-owned | Existing object storage tests and B032 governance scans |
| CODEX-0746 | supplemental-requirement | `codex-prompts/08-platform-infrastructure/P0031.md` -> `observability` constraints | No Rust ownership; merge observability constraints into primary/test coverage | `observability_impl_contract_tests.rs` |
| CODEX-0747 | supplemental-requirement | `codex-prompts/08-platform-infrastructure/P0032.md` -> `performance_budget` constraints | No Rust ownership; performance-budget constraints remain B031-owned | Existing performance budget tests and B032 governance scans |
| CODEX-0748 | supplemental-requirement | `codex-prompts/08-platform-infrastructure/P0033.md` -> `readme` constraints | No Rust ownership; invariant constraints remain B031-owned | Existing readme tests and B032 evidence |
| CODEX-0749 | supplemental-requirement | `codex-prompts/08-platform-infrastructure/P0036.md` -> `reliability_performance` constraints | No Rust ownership; merge into reliability impl contract | `reliability_performance_impl_contract_tests.rs` |
| CODEX-0750 | supplemental-requirement | `codex-prompts/08-platform-infrastructure/P0035.md` -> `observability` constraints | No Rust ownership; merge into observability impl contract | `observability_impl_contract_tests.rs` |
| CODEX-0751 | supplemental-requirement | `codex-prompts/08-platform-infrastructure/P0034.md` -> `deployment_ops` constraints | No Rust ownership; merge into deployment ops impl contract | `deployment_ops_impl_contract_tests.rs` |
| CODEX-0752 | primary-implementation | `crates/trpg-platform/src/api_contracts_impl.rs` and `tests/api_contracts_impl_contract_tests.rs` | Add domain command/event/service/repository/error types and event-store handler | Authority violation, visibility replay, fact provenance, metric module |
| CODEX-0753 | primary-implementation | `crates/trpg-platform/src/deployment_ops_impl.rs` and `tests/deployment_ops_impl_contract_tests.rs` | Add deployment operation event contract; reuse provider boundary | Authority violation, visibility/provenance, public unauthenticated local provider denial |
| CODEX-0754 | primary-implementation | `crates/trpg-platform/src/observability_impl.rs` and `tests/observability_impl_contract_tests.rs` | Add platform observation event contract and restricted-detail redaction | Authority violation, visibility/provenance, metric prefix denial |
| CODEX-0755 | primary-implementation | `crates/trpg-platform/src/plugin_sdk_impl.rs` and `tests/plugin_sdk_impl_contract_tests.rs` | Add plugin tool grant contract; deny direct formal-state write grants | Authority violation, visibility/provenance, direct write grant denial |
| CODEX-0756 | primary-implementation | `crates/trpg-platform/src/policy_authz_impl.rs` and `tests/policy_authz_impl_contract_tests.rs` | Add policy authorization contract; enforce deny decisions fail closed | Authority violation, visibility/provenance, deny decision |
| CODEX-0757 | primary-implementation | `crates/trpg-platform/src/reliability_performance_impl.rs` and `tests/reliability_performance_impl_contract_tests.rs` | Add reliability/performance guard contract; reuse retry delay policy | Authority violation, visibility/provenance, capped retry evidence |
| CODEX-0758 | primary-implementation | `crates/trpg-platform/src/security_privacy_copyrightmpl.rs` and `tests/security_privacy_copyrightmpl_contract_tests.rs` | Add security/privacy/copyright review contract; deny restricted visibility export | Authority violation, visibility/provenance, restricted export denial |
| CODEX-0759 | documentation-or-traceability | `docs/codex/08-platform-infrastructure/source_processing_record_docs_implementation_08_platform_infrastructure_local_dev_environment.md` | Traceability only; no code ownership | Covered by B032 evidence |
| CODEX-0760 | documentation-or-traceability | `docs/codex/08-platform-infrastructure/source_processing_record_docs_implementation_08_platform_infrastructure_background_workers.md` | Traceability only; no code ownership | Covered by B032 evidence |
| CODEX-0761 | documentation-or-traceability | `docs/codex/08-platform-infrastructure/source_processing_record_docs_implementation_08_platform_infrastructure_observability.md` | Traceability only; no code ownership | Covered by B032 evidence |
| CODEX-0762 | documentation-or-traceability | `docs/codex/08-platform-infrastructure/source_processing_record_docs_implementation_08_platform_infrastructure_deployment_ops.md` | Traceability only; no code ownership | Covered by B032 evidence |
| CODEX-0763 | documentation-or-traceability | `docs/codex/08-platform-infrastructure/source_processing_record_docs_implementation_08_platform_infrastructure_readme.md` | Traceability only; no code ownership | Covered by B032 evidence |
| CODEX-0764 | documentation-or-traceability | `docs/codex/08-platform-infrastructure/source_processing_record_docs_implementation_08_platform_infrastructure_object_storage.md` | Traceability only; no code ownership | Covered by B032 evidence |
| CODEX-0765 | documentation-or-traceability | `docs/codex/08-platform-infrastructure/source_processing_record_docs_implementation_08_platform_infrastructure_performance_budget.md` | Traceability only; no code ownership | Covered by B032 evidence |
