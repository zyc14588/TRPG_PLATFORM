# BATCH-038 Work Plan

Batch: `BATCH-038-10-testing-quality`  
Stage: `S11 testing quality golden CI`  
Declared prompt count: 25

Authority note: the user-provided batch facts say `recognized primary prompt count: 0`, but `batches/B038.md`, the normalized execution map, and the current-safe output map identify 12 primary implementation prompts, 11 supplemental prompts, and 2 documentation/traceability prompts. This execution follows the repository authority order and records the conflict here.

## Prompt Mapping

| Prompt ID | Prompt file | Target file or module | Allowed scope | Test responsibility |
| --- | --- | --- | --- | --- |
| CODEX-0088-10-TESTING-QUALITY-20897e8633 | P0004 | `docs/codex/10-testing-quality/m_10_testing_quality.md` | Documentation/traceability only | Covered by B038 evidence review |
| CODEX-0089-10-TESTING-QUALITY-da28af3028 | P0001 | `crates/trpg-testing/src/benchmark_plan.rs`; `crates/trpg-testing/tests/benchmark_plan_contract_tests.rs` | Primary implementation | Benchmark plan command/event contract and budget coverage |
| CODEX-0090-10-TESTING-QUALITY-db69d85d0f | P0002 | `docs/codex/10-testing-quality/contract_test_matrix.md` | Documentation/traceability only | Covered by contract matrix source test |
| CODEX-0091-10-TESTING-QUALITY-6730499fe0 | P0003 | `crates/trpg-testing/src/model_certification_tests.rs`; `crates/trpg-testing/tests/model_certification_tests_contract_tests.rs` | Primary implementation | Local model Level 4 gate and silent fallback denial |
| CODEX-0092-10-TESTING-QUALITY-d6a006e0a1 | P0005 | `crates/trpg-testing/src/replay_consistency_tests.rs`; `crates/trpg-testing/tests/replay_consistency_tests_contract_tests.rs` | Primary implementation | Deterministic replay and projection rebuild contract |
| CODEX-0093-10-TESTING-QUALITY-97f7f731a8 | P0006 | `crates/trpg-testing/src/test_strategy.rs`; `crates/trpg-testing/tests/test_strategy_contract_tests.rs` | Primary implementation | Test layer strategy and fixture coverage contract |
| CODEX-0094-10-TESTING-QUALITY-6ac95ec41f | P0007 | `crates/trpg-testing/src/testing_golden_ci.rs`; `crates/trpg-testing/tests/testing_golden_ci_contract_tests.rs` | Primary implementation | Golden CI gate and stage fixture contract |
| CODEX-0095-10-TESTING-QUALITY-e84e4a394d | P0008 | `crates/trpg-testing/src/visibility_leakage_tests.rs`; `crates/trpg-testing/tests/visibility_leakage_tests_contract_tests.rs` | Primary implementation | Restricted visibility redaction and no player leakage |
| CODEX-0839-10-TESTING-QUALITY-09775e3a7b | P0009 | `crates/trpg-testing/src/decision_trace_map.rs`; `crates/trpg-testing/tests/decision_trace_map_contract_tests.rs` | Primary implementation | Decision trace map coverage and current-safe naming |
| CODEX-0840-10-TESTING-QUALITY-069d3f779b | P0010 | Supplemental requirement for `decision_trace_map` | Supplemental requirements only | Merged into decision trace map contract test |
| CODEX-0841-10-TESTING-QUALITY-661dfc0224 | P0011 | Supplemental requirement for `benchmark_plan` | Supplemental requirements only | Merged into benchmark plan contract test |
| CODEX-0842-10-TESTING-QUALITY-70ddb67f5e | P0012 | `crates/trpg-testing/src/contract_test_matrix.rs`; `crates/trpg-testing/tests/contract_test_matrix_contract_tests.rs` | Primary implementation | Contract matrix row coverage |
| CODEX-0843-10-TESTING-QUALITY-a8f283084f | P0014 | Supplemental requirement for `testing_golden_ci` | Supplemental requirements only | Merged into golden CI contract test |
| CODEX-0844-10-TESTING-QUALITY-be04cff75f | P0015 | Supplemental requirement for `test_strategy` | Supplemental requirements only | Merged into test strategy contract test |
| CODEX-0845-10-TESTING-QUALITY-6254d78940 | P0013 | `crates/trpg-testing/src/testing_golden_scenarios_ci.rs`; `crates/trpg-testing/tests/testing_golden_scenarios_ci_contract_tests.rs` | Primary implementation | Golden scenario fixture/action coverage |
| CODEX-0846-10-TESTING-QUALITY-4191e3f193 | P0016 | `crates/trpg-testing/src/golden_scenario_ci.rs`; `crates/trpg-testing/tests/golden_scenario_ci_contract_tests.rs` | Primary implementation | Golden scenario smoke gate and official decision path |
| CODEX-0847-10-TESTING-QUALITY-923fc94916 | P0017 | Supplemental requirement for `test_strategy` | Supplemental requirements only | Merged into test strategy contract test |
| CODEX-0848-10-TESTING-QUALITY-85ad4a2b62 | P0018 | `crates/trpg-testing/src/implementation_acceptance_checklist.rs`; `crates/trpg-testing/tests/implementation_acceptance_checklist_contract_tests.rs` | Primary implementation | Acceptance checklist coverage |
| CODEX-0849-10-TESTING-QUALITY-3b745596ac | P0021 | Supplemental requirement for `benchmark_plan` | Supplemental requirements only | Merged into benchmark plan contract test |
| CODEX-0850-10-TESTING-QUALITY-f5b7059f4f | P0025 | Supplemental requirement for `contract_test_matrix` | Supplemental requirements only | Merged into contract matrix contract test |
| CODEX-0851-10-TESTING-QUALITY-c4d5125cc0 | P0023 | Supplemental requirement for `model_certification_tests` | Supplemental requirements only | Merged into model certification contract test |
| CODEX-0852-10-TESTING-QUALITY-1afba0632b | P0020 | `crates/trpg-testing/src/readme.rs`; `crates/trpg-testing/tests/readme_contract_tests.rs` | Primary implementation | Readme module contract and required metrics |
| CODEX-0853-10-TESTING-QUALITY-eaf9de3475 | P0024 | Supplemental requirement for `replay_consistency_tests` | Supplemental requirements only | Merged into replay consistency contract test |
| CODEX-0854-10-TESTING-QUALITY-705d02fdf8 | P0022 | Supplemental requirement for `test_strategy` | Supplemental requirements only | Merged into test strategy contract test |
| CODEX-0855-10-TESTING-QUALITY-0adc8f6280 | P0019 | Supplemental requirement for `testing_golden_ci` | Supplemental requirements only | Merged into golden CI contract test |

## Execution Slice

1. Add a minimal `trpg-testing` workspace crate using existing shared-kernel and agent-runtime contracts.
2. Implement current-safe testing-quality module entry points and one shared event-backed testing quality contract.
3. Add focused contract tests for each primary prompt-owned module.
4. Add documentation outputs and supplemental requirement records without changing prompt files.
5. Run minimal `trpg-testing` checks first, then available S11 stage checks.
