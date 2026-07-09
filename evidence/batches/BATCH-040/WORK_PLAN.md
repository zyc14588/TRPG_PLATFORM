# BATCH-040 Work Plan

Batch: `BATCH-040-10-testing-quality`
Stage: `S11-testing-quality-golden-ci`
Declared prompts: 25
Primary prompts found in B040: 2 (`CODEX-0892`, `CODEX-0893`)

## Scope Rule

Only B040 outputs are in scope. Historical V3/V4/V5/V6 path fragments remain provenance and are not used for current module, test, metric, event schema, migration, workflow, or output names.

## Prompt Map

| Prompt ID | Prompt | Role | Current-safe target | Allowed change | Test responsibility |
| --- | --- | --- | --- | --- | --- |
| `CODEX-0881-10-TESTING-QUALITY-ab5a85e024` | `P0053.md` | documentation-or-traceability | `docs/codex/10-testing-quality/source_processing_record_docs_implementation_10_testing_quality_benchmark_plan.md` | Markdown trace only | documentation self-check |
| `CODEX-0882-10-TESTING-QUALITY-bce7281791` | `P0055.md` | documentation-or-traceability | `docs/codex/10-testing-quality/source_processing_record_docs_implementation_10_testing_quality_replay_consistency_tests.md` | Markdown trace only | documentation self-check |
| `CODEX-0883-10-TESTING-QUALITY-a86c3e6648` | `P0052.md` | documentation-or-traceability | `docs/codex/10-testing-quality/source_processing_record_docs_implementation_10_testing_quality_testing_golden_ci.md` | Markdown trace only | documentation self-check |
| `CODEX-0884-10-TESTING-QUALITY-1ac29837fe` | `P0048.md` | documentation-or-traceability | `docs/codex/10-testing-quality/source_processing_record_docs_implementation_10_testing_quality_test_strategy.md` | Markdown trace only | documentation self-check |
| `CODEX-0885-10-TESTING-QUALITY-c268ac4060` | `P0057.md` | documentation-or-traceability | `docs/codex/10-testing-quality/source_processing_record_docs_implementation_90_traceability_source_breakdown_testing_test_strategy.md` | Markdown trace only | documentation self-check |
| `CODEX-0886-10-TESTING-QUALITY-16dc13a1c3` | `P0047.md` | documentation-or-traceability | `docs/codex/10-testing-quality/source_processing_record_docs_implementation_90_traceability_source_breakdown_testing_golden_scenarios_ci.md` | Markdown trace only | documentation self-check |
| `CODEX-0887-10-TESTING-QUALITY-c0507ea620` | `P0056.md` | documentation-or-traceability | `docs/codex/10-testing-quality/source_processing_record_docs_testing_golden_scenarios_ci.md` | Markdown trace only | documentation self-check |
| `CODEX-0888-10-TESTING-QUALITY-5f13aab48b` | `P0050.md` | documentation-or-traceability | `docs/codex/10-testing-quality/source_processing_record_docs_testing_test_strategy.md` | Markdown trace only | documentation self-check |
| `CODEX-0889-10-TESTING-QUALITY-1a73fc55df` | `P0059.md` | documentation-or-traceability | `docs/codex/10-testing-quality/source_processing_record_docs_implementation_90_traceability_requirement_to_test_trace.md` | Markdown trace only | documentation self-check |
| `CODEX-0890-10-TESTING-QUALITY-6c42e0adc6` | `P0060.md` | documentation-or-traceability | `docs/codex/10-testing-quality/source_processing_record_docs_implementation_90_traceability_top_level_principle_trace.md` | Markdown trace only | documentation self-check |
| `CODEX-0891-10-TESTING-QUALITY-bfdb55657b` | `P0061.md` | supplemental-requirement | `docs/codex/90-traceability/supplemental-requirements/CODEX-0891-10-TESTING-QUALITY-bfdb55657b.md` | Supplemental Markdown only | merged into `top_level_principle_trace` tests |
| `CODEX-0892-10-TESTING-QUALITY-1b68a77fb7` | `P0062.md` | primary-implementation | `crates/trpg-testing/src/latest_deep_research_rust_summary.rs` and contract test | Current-safe Rust contract module | `latest_deep_research_rust_summary_contract_tests` |
| `CODEX-0893-10-TESTING-QUALITY-d9f4b3d265` | `P0063.md` | primary-implementation | `crates/trpg-testing/src/research_decision_matrix.rs` and contract test | Current-safe Rust contract module | `research_decision_matrix_contract_tests` |
| `CODEX-0894-10-TESTING-QUALITY-cbce2d32dc` | `P0064.md` | supplemental-requirement | `docs/codex/90-traceability/supplemental-requirements/CODEX-0894-10-TESTING-QUALITY-cbce2d32dc.md` | Supplemental Markdown only | merged into `decision_trace_map` tests |
| `CODEX-0895-10-TESTING-QUALITY-da41f3501a` | `P0065.md` | supplemental-requirement | `docs/codex/90-traceability/supplemental-requirements/CODEX-0895-10-TESTING-QUALITY-da41f3501a.md` | Supplemental Markdown only | merged into `benchmark_plan` tests |
| `CODEX-0896-10-TESTING-QUALITY-8f3eea7a40` | `P0066.md` | supplemental-requirement | `docs/codex/90-traceability/supplemental-requirements/CODEX-0896-10-TESTING-QUALITY-8f3eea7a40.md` | Supplemental Markdown only | merged into `contract_test_matrix` tests |
| `CODEX-0897-10-TESTING-QUALITY-343bdf6c01` | `P0067.md` | supplemental-requirement | `docs/codex/90-traceability/supplemental-requirements/CODEX-0897-10-TESTING-QUALITY-343bdf6c01.md` | Supplemental Markdown only | merged into `model_certification_tests` tests |
| `CODEX-0898-10-TESTING-QUALITY-236e6fd7c8` | `P0068.md` | supplemental-requirement | `docs/codex/90-traceability/supplemental-requirements/CODEX-0898-10-TESTING-QUALITY-236e6fd7c8.md` | Supplemental Markdown only | merged into `readme` tests |
| `CODEX-0899-10-TESTING-QUALITY-8afffdc3be` | `P0069.md` | supplemental-requirement | `docs/codex/90-traceability/supplemental-requirements/CODEX-0899-10-TESTING-QUALITY-8afffdc3be.md` | Supplemental Markdown only | merged into `replay_consistency_tests` tests |
| `CODEX-0900-10-TESTING-QUALITY-8546747a83` | `P0071.md` | supplemental-requirement | `docs/codex/90-traceability/supplemental-requirements/CODEX-0900-10-TESTING-QUALITY-8546747a83.md` | Supplemental Markdown only | merged into `test_strategy` tests |
| `CODEX-0901-10-TESTING-QUALITY-45a7ba08ee` | `P0070.md` | supplemental-requirement | `docs/codex/90-traceability/supplemental-requirements/CODEX-0901-10-TESTING-QUALITY-45a7ba08ee.md` | Supplemental Markdown only | merged into `testing_golden_ci` tests |
| `CODEX-0902-10-TESTING-QUALITY-b150cdeefc` | `P0072.md` | supplemental-requirement | `docs/codex/90-traceability/supplemental-requirements/CODEX-0902-10-TESTING-QUALITY-b150cdeefc.md` | Supplemental Markdown only | merged into `requirement_to_test_trace` tests |
| `CODEX-0903-10-TESTING-QUALITY-7c2a0eab6e` | `P0074.md` | supplemental-requirement | `docs/codex/90-traceability/supplemental-requirements/CODEX-0903-10-TESTING-QUALITY-7c2a0eab6e.md` | Supplemental Markdown only | merged into `golden_scenarios_ci_impl` tests |
| `CODEX-0904-10-TESTING-QUALITY-0bc9c5503b` | `P0073.md` | supplemental-requirement | `docs/codex/90-traceability/supplemental-requirements/CODEX-0904-10-TESTING-QUALITY-0bc9c5503b.md` | Supplemental Markdown only | merged into `test_strategy_impl` tests |
| `CODEX-0905-10-TESTING-QUALITY-d5c5ad6036` | `P0075.md` | supplemental-requirement | `docs/codex/90-traceability/supplemental-requirements/CODEX-0905-10-TESTING-QUALITY-d5c5ad6036.md` | Supplemental Markdown only | merged into `top_level_principle_trace` tests |

## Planned Checks

1. Minimal related: `cargo test -p trpg-testing --all-features`
2. Stage checks: `cargo test -p trpg-testing --test golden_scenarios_ci --all-features`; `cargo test -p trpg-testing --test visibility_leakage --all-features`; `cargo test -p trpg-testing --test model_certification_tests --all-features`
3. Formatting: `cargo fmt --all -- --check`

## Risk

The user-supplied batch fact said primary prompt count was 0, while active B040 and normalized maps identify two B040 primary prompts. This run follows active B040/current-safe mapping and records the discrepancy here.
