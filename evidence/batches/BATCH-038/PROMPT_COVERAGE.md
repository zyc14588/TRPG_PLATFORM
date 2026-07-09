# BATCH-038 Prompt Coverage

Batch: `BATCH-038-10-testing-quality`  
Declared prompt count: 25  
Implemented primary prompts: 12  
Documentation prompts: 2  
Supplemental prompts: 11

## Primary Implementation Coverage

| Prompt ID | Current-safe module | Source | Test |
| --- | --- | --- | --- |
| CODEX-0089-10-TESTING-QUALITY-da28af3028 | `testing_quality::benchmark_plan` | `crates/trpg-testing/src/benchmark_plan.rs` | `crates/trpg-testing/tests/benchmark_plan_contract_tests.rs` |
| CODEX-0091-10-TESTING-QUALITY-6730499fe0 | `testing_quality::model_certification_tests` | `crates/trpg-testing/src/model_certification_tests.rs` | `crates/trpg-testing/tests/model_certification_tests_contract_tests.rs` |
| CODEX-0092-10-TESTING-QUALITY-d6a006e0a1 | `testing_quality::replay_consistency_tests` | `crates/trpg-testing/src/replay_consistency_tests.rs` | `crates/trpg-testing/tests/replay_consistency_tests_contract_tests.rs` |
| CODEX-0093-10-TESTING-QUALITY-97f7f731a8 | `testing_quality::test_strategy` | `crates/trpg-testing/src/test_strategy.rs` | `crates/trpg-testing/tests/test_strategy_contract_tests.rs` |
| CODEX-0094-10-TESTING-QUALITY-6ac95ec41f | `testing_quality::testing_golden_ci` | `crates/trpg-testing/src/testing_golden_ci.rs` | `crates/trpg-testing/tests/testing_golden_ci_contract_tests.rs` |
| CODEX-0095-10-TESTING-QUALITY-e84e4a394d | `testing_quality::visibility_leakage_tests` | `crates/trpg-testing/src/visibility_leakage_tests.rs` | `crates/trpg-testing/tests/visibility_leakage_tests_contract_tests.rs` |
| CODEX-0839-10-TESTING-QUALITY-09775e3a7b | `testing_quality::decision_trace_map` | `crates/trpg-testing/src/decision_trace_map.rs` | `crates/trpg-testing/tests/decision_trace_map_contract_tests.rs` |
| CODEX-0842-10-TESTING-QUALITY-70ddb67f5e | `testing_quality::contract_test_matrix` | `crates/trpg-testing/src/contract_test_matrix.rs` | `crates/trpg-testing/tests/contract_test_matrix_contract_tests.rs` |
| CODEX-0845-10-TESTING-QUALITY-6254d78940 | `testing_quality::testing_golden_scenarios_ci` | `crates/trpg-testing/src/testing_golden_scenarios_ci.rs` | `crates/trpg-testing/tests/testing_golden_scenarios_ci_contract_tests.rs` |
| CODEX-0846-10-TESTING-QUALITY-4191e3f193 | `testing_quality::golden_scenario_ci` | `crates/trpg-testing/src/golden_scenario_ci.rs` | `crates/trpg-testing/tests/golden_scenario_ci_contract_tests.rs` |
| CODEX-0848-10-TESTING-QUALITY-85ad4a2b62 | `testing_quality::implementation_acceptance_checklist` | `crates/trpg-testing/src/implementation_acceptance_checklist.rs` | `crates/trpg-testing/tests/implementation_acceptance_checklist_contract_tests.rs` |
| CODEX-0852-10-TESTING-QUALITY-1afba0632b | `testing_quality::readme` | `crates/trpg-testing/src/readme.rs` | `crates/trpg-testing/tests/readme_contract_tests.rs` |

## Documentation Coverage

| Prompt ID | Output |
| --- | --- |
| CODEX-0088-10-TESTING-QUALITY-20897e8633 | `docs/codex/10-testing-quality/m_10_testing_quality.md` |
| CODEX-0090-10-TESTING-QUALITY-db69d85d0f | `docs/codex/10-testing-quality/contract_test_matrix.md` |

## Supplemental Coverage

| Prompt ID | Primary merge target | Evidence output |
| --- | --- | --- |
| CODEX-0840-10-TESTING-QUALITY-069d3f779b | `testing_quality::decision_trace_map` | `docs/codex/90-traceability/supplemental-requirements/CODEX-0840-10-TESTING-QUALITY-069d3f779b.md` |
| CODEX-0841-10-TESTING-QUALITY-661dfc0224 | `testing_quality::benchmark_plan` | `docs/codex/90-traceability/supplemental-requirements/CODEX-0841-10-TESTING-QUALITY-661dfc0224.md` |
| CODEX-0843-10-TESTING-QUALITY-a8f283084f | `testing_quality::testing_golden_ci` | `docs/codex/90-traceability/supplemental-requirements/CODEX-0843-10-TESTING-QUALITY-a8f283084f.md` |
| CODEX-0844-10-TESTING-QUALITY-be04cff75f | `testing_quality::test_strategy` | `docs/codex/90-traceability/supplemental-requirements/CODEX-0844-10-TESTING-QUALITY-be04cff75f.md` |
| CODEX-0847-10-TESTING-QUALITY-923fc94916 | `testing_quality::test_strategy` | `docs/codex/90-traceability/supplemental-requirements/CODEX-0847-10-TESTING-QUALITY-923fc94916.md` |
| CODEX-0849-10-TESTING-QUALITY-3b745596ac | `testing_quality::benchmark_plan` | `docs/codex/90-traceability/supplemental-requirements/CODEX-0849-10-TESTING-QUALITY-3b745596ac.md` |
| CODEX-0850-10-TESTING-QUALITY-f5b7059f4f | `testing_quality::contract_test_matrix` | `docs/codex/90-traceability/supplemental-requirements/CODEX-0850-10-TESTING-QUALITY-f5b7059f4f.md` |
| CODEX-0851-10-TESTING-QUALITY-c4d5125cc0 | `testing_quality::model_certification_tests` | `docs/codex/90-traceability/supplemental-requirements/CODEX-0851-10-TESTING-QUALITY-c4d5125cc0.md` |
| CODEX-0853-10-TESTING-QUALITY-eaf9de3475 | `testing_quality::replay_consistency_tests` | `docs/codex/90-traceability/supplemental-requirements/CODEX-0853-10-TESTING-QUALITY-eaf9de3475.md` |
| CODEX-0854-10-TESTING-QUALITY-705d02fdf8 | `testing_quality::test_strategy` | `docs/codex/90-traceability/supplemental-requirements/CODEX-0854-10-TESTING-QUALITY-705d02fdf8.md` |
| CODEX-0855-10-TESTING-QUALITY-0adc8f6280 | `testing_quality::testing_golden_ci` | `docs/codex/90-traceability/supplemental-requirements/CODEX-0855-10-TESTING-QUALITY-0adc8f6280.md` |

Stage alias tests added for S11 TEST_PLAN command targets:

- `crates/trpg-testing/tests/golden_scenarios_ci.rs`
- `crates/trpg-testing/tests/visibility_leakage.rs`
- `crates/trpg-testing/tests/model_certification_tests.rs`
