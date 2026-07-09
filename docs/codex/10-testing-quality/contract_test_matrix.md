# Contract Test Matrix

Batch: `BATCH-038-10-testing-quality`  
Current crate: `trpg-testing`

| Requirement | Current-safe module | Contract test |
| --- | --- | --- |
| Benchmark plan and budget gates | `testing_quality::benchmark_plan` | `crates/trpg-testing/tests/benchmark_plan_contract_tests.rs` |
| Local model Level 4 and fallback gate | `testing_quality::model_certification_tests` | `crates/trpg-testing/tests/model_certification_tests_contract_tests.rs` |
| Event replay and projection rebuild | `testing_quality::replay_consistency_tests` | `crates/trpg-testing/tests/replay_consistency_tests_contract_tests.rs` |
| Layered S11 test strategy | `testing_quality::test_strategy` | `crates/trpg-testing/tests/test_strategy_contract_tests.rs` |
| Golden CI stage gate | `testing_quality::testing_golden_ci` | `crates/trpg-testing/tests/testing_golden_ci_contract_tests.rs` |
| Visibility leakage redaction | `testing_quality::visibility_leakage_tests` | `crates/trpg-testing/tests/visibility_leakage_tests_contract_tests.rs` |
| Decision trace map | `testing_quality::decision_trace_map` | `crates/trpg-testing/tests/decision_trace_map_contract_tests.rs` |
| Matrix row coverage | `testing_quality::contract_test_matrix` | `crates/trpg-testing/tests/contract_test_matrix_contract_tests.rs` |
| Golden scenario fixture suite | `testing_quality::testing_golden_scenarios_ci` | `crates/trpg-testing/tests/testing_golden_scenarios_ci_contract_tests.rs` |
| Golden scenario CI decision path | `testing_quality::golden_scenario_ci` | `crates/trpg-testing/tests/golden_scenario_ci_contract_tests.rs` |
| Implementation acceptance checklist | `testing_quality::implementation_acceptance_checklist` | `crates/trpg-testing/tests/implementation_acceptance_checklist_contract_tests.rs` |
| Testing-quality readme/metrics | `testing_quality::readme` | `crates/trpg-testing/tests/readme_contract_tests.rs` |

Supplemental prompts in B038 do not own Rust modules or migrations. They are recorded under `docs/codex/90-traceability/supplemental-requirements/` and their test responsibility is merged into the primary module named in `batches/B038.md`.
