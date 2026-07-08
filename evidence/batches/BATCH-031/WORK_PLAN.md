# BATCH-031 Work Plan

Batch: `BATCH-031-08-platform-infrastructure -- Strict Governance Final`

Normalized maps applied before execution:

- `docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`
- `docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md`
- `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`

Scope rule: only B031 outputs are implemented. `source-archive/**` remains provenance only.

| Prompt ID | Role | Current-safe target | Allowed change | Test responsibility |
|---|---|---|---|---|
| CODEX-0073 | documentation-or-traceability | `docs/codex/08-platform-infrastructure/m_08_platform_infrastructure.md` | Create current-safe module traceability index; no code ownership | Covered by this batch evidence |
| CODEX-0074 | primary | `crates/trpg-platform/src/background_workers.rs` | Worker command/event contract and governance guard | `background_workers_contract_tests.rs` |
| CODEX-0075 | primary | `crates/trpg-platform/src/deployment_ops.rs` | Deployment provider boundary and config event | `deployment_ops_contract_tests.rs` |
| CODEX-0076 | primary | `crates/trpg-platform/src/local_dev_environment.rs` | Local dev loopback validation and event | `local_dev_environment_contract_tests.rs` |
| CODEX-0077 | primary | `crates/trpg-platform/src/object_storage.rs` | Object descriptor visibility redaction and event | `object_storage_contract_tests.rs` |
| CODEX-0078 | primary | `crates/trpg-platform/src/observability.rs` | Metric naming and restricted-detail redaction | `observability_contract_tests.rs` |
| CODEX-0079 | primary | `crates/trpg-platform/src/performance_budget.rs` | Budget fail-closed check and event | `performance_budget_contract_tests.rs` |
| CODEX-0723 | supplemental | `codex-prompts/08-platform-infrastructure/P0008-*.md` | Constraint merged into background worker tests/evidence only | `background_workers_contract_tests.rs` |
| CODEX-0724 | primary | `crates/trpg-platform/src/deployment_observability.rs` | Deployment health observation contract | `deployment_observability_contract_tests.rs` |
| CODEX-0725 | supplemental | `codex-prompts/08-platform-infrastructure/P0010-*.md` | Constraint merged into deployment ops tests/evidence only | `deployment_ops_contract_tests.rs` |
| CODEX-0726 | supplemental | `codex-prompts/08-platform-infrastructure/P0024-*.md` | Constraint merged into observability tests/evidence only | `observability_contract_tests.rs` |
| CODEX-0727 | supplemental | `codex-prompts/08-platform-infrastructure/P0016-*.md` | Constraint merged into deployment ops tests/evidence only | `deployment_ops_contract_tests.rs` |
| CODEX-0728 | primary | `crates/trpg-platform/src/reliability_performance.rs` | Retry/backoff reliability contract | `reliability_performance_contract_tests.rs` |
| CODEX-0729 | supplemental | `codex-prompts/08-platform-infrastructure/P0022-*.md` | Constraint merged into observability tests/evidence only | `observability_contract_tests.rs` |
| CODEX-0730 | supplemental | `codex-prompts/08-platform-infrastructure/P0020-*.md` | Constraint merged into deployment ops tests/evidence only | `deployment_ops_contract_tests.rs` |
| CODEX-0731 | supplemental | `codex-prompts/08-platform-infrastructure/P0019-*.md` | Constraint merged into deployment ops tests/evidence only | `deployment_ops_contract_tests.rs` |
| CODEX-0732 | primary | `crates/trpg-platform/src/observability_audit_trace.rs` | Audit trace redaction and command metadata guard | `observability_audit_trace_contract_tests.rs` |
| CODEX-0733 | supplemental | `codex-prompts/08-platform-infrastructure/P0011-*.md` | Constraint merged into reliability tests/evidence only | `reliability_performance_contract_tests.rs` |
| CODEX-0734 | supplemental | `codex-prompts/08-platform-infrastructure/P0027-*.md` | Constraint merged into deployment ops tests/evidence only | `deployment_ops_contract_tests.rs` |
| CODEX-0735 | supplemental | `codex-prompts/08-platform-infrastructure/P0018-*.md` | Constraint merged into local dev tests/evidence only | `local_dev_environment_contract_tests.rs` |
| CODEX-0736 | supplemental | `codex-prompts/08-platform-infrastructure/P0013-*.md` | Constraint merged into background worker tests/evidence only | `background_workers_contract_tests.rs` |
| CODEX-0737 | supplemental | `codex-prompts/08-platform-infrastructure/P0025-*.md` | Constraint merged into observability tests/evidence only | `observability_contract_tests.rs` |
| CODEX-0738 | primary | `crates/trpg-platform/src/readme.rs` | Platform invariant summary and event | `readme_contract_tests.rs` |
| CODEX-0739 | supplemental | `codex-prompts/08-platform-infrastructure/P0023-*.md` | Constraint merged into object storage tests/evidence only | `object_storage_contract_tests.rs` |
| CODEX-0740 | supplemental | `codex-prompts/08-platform-infrastructure/P0014-*.md` | Constraint merged into performance budget tests/evidence only | `performance_budget_contract_tests.rs` |
