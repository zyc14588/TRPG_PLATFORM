# BATCH-039-10-testing-quality Work Plan

Batch: `BATCH-039-10-testing-quality`
Stage: `stages/s11-testing-quality-golden-ci`
Scope: strict governance testing-quality harness and traceability records only.

## Boundary Notes

- `source-archive/**` and historical source names are provenance only.
- Supplemental prompts in this batch do not own Rust output. They are treated as merge constraints on the named current-safe primary modules.
- Rust changes are limited to `crates/trpg-testing` current-safe modules and matching contract tests.
- Documentation changes are limited to the five current-safe source processing records and this batch evidence folder.

## Prompt Map

| Prompt ID | Prompt | Role | Current-safe target | Allowed change | Test responsibility |
|---|---|---|---|---|---|
| `CODEX-0856-10-TESTING-QUALITY-78184e52c9` | `P0027.md` | supplemental-requirement | `testing_quality::golden_scenario_ci` | Constraint only; no new Rust ownership. | Covered by existing `golden_scenario_ci_contract_tests` and S11 golden scenario checks. |
| `CODEX-0857-10-TESTING-QUALITY-2aa3aea6d1` | `P0026.md` | supplemental-requirement | `testing_quality::test_strategy` | Constraint only; no new Rust ownership. | Covered by existing `test_strategy_contract_tests` and S11 test strategy checks. |
| `CODEX-0858-10-TESTING-QUALITY-dae7b4dc49` | `P0028.md` | primary-implementation | `crates/trpg-testing/src/golden_ci_test_matrix.rs` | Add current-safe test matrix module and matching contract test. | Verify golden CI gates, visibility export diff, provider certification, and event path assertions. |
| `CODEX-0859-10-TESTING-QUALITY-e25a4b4478` | `P0029.md` | primary-implementation | `crates/trpg-testing/src/implementation_acceptance_checklist_source_contract.rs` | Add checklist source-contract module and matching contract test. | Verify acceptance source contract, zero P0/P1 rule, visibility/provenance checks. |
| `CODEX-0860-10-TESTING-QUALITY-0de1e4e40c` | `P0030.md` | supplemental-requirement | `testing_quality::model_certification_tests` | Constraint only; no new Rust ownership. | Covered by existing model certification contract and stage test. |
| `CODEX-0861-10-TESTING-QUALITY-2664a3d8ee` | `P0031.md` | supplemental-requirement | `testing_quality::readme` | Constraint only; no new Rust ownership. | Covered by existing README contract test. |
| `CODEX-0862-10-TESTING-QUALITY-fdd4d14b4b` | `P0032.md` | supplemental-requirement | `testing_quality::replay_consistency_tests` | Constraint only; no new Rust ownership. | Covered by existing replay consistency contract test. |
| `CODEX-0863-10-TESTING-QUALITY-0c4693f8ae` | `P0033.md` | supplemental-requirement | `testing_quality::test_strategy` | Constraint only; no new Rust ownership. | Covered by existing and new test strategy tests. |
| `CODEX-0864-10-TESTING-QUALITY-6cb8a48c80` | `P0034.md` | supplemental-requirement | `testing_quality::testing_golden_ci` | Constraint only; no new Rust ownership. | Covered by existing golden CI contract test. |
| `CODEX-0865-10-TESTING-QUALITY-aea366b339` | `P0040.md` | primary-implementation | `crates/trpg-testing/src/requirement_to_test_trace.rs` | Add requirement-to-test trace module and matching contract test. | Verify V1/S11 requirements map to commands, tests, and evidence paths. |
| `CODEX-0866-10-TESTING-QUALITY-897bc79dc9` | `P0039.md` | primary-implementation | `crates/trpg-testing/src/ai_evaluation_golden_scenario.rs` | Add AI evaluation golden scenario module and matching contract test. | Verify AI evaluation remains tool-gated, visibility-aware, and event-recorded. |
| `CODEX-0867-10-TESTING-QUALITY-7667081407` | `P0038.md` | supplemental-requirement | `testing_quality::test_strategy` | Constraint only; no new Rust ownership. | Covered by existing and new test strategy tests. |
| `CODEX-0868-10-TESTING-QUALITY-14936fa877` | `P0036.md` | primary-implementation | `crates/trpg-testing/src/runtime_pending_decision.rs` | Add pending decision runtime module and matching contract test. | Verify pending decisions cannot mutate Authority Contract or bypass Event Store. |
| `CODEX-0869-10-TESTING-QUALITY-20c4bd1d75` | `P0037.md` | supplemental-requirement | `testing_quality::testing_golden_scenarios_ci` | Constraint only; no new Rust ownership. | Covered by existing golden scenarios CI contract test. |
| `CODEX-0870-10-TESTING-QUALITY-0142acfd95` | `P0035.md` | primary-implementation | `crates/trpg-testing/src/top_level_principle_trace.rs` | Add top-level principle trace module and matching contract test. | Verify Authority, Agent Gateway, Event Store, Visibility, and Provider boundary trace rows. |
| `CODEX-0871-10-TESTING-QUALITY-fd5bd618c9` | `P0041.md` | primary-implementation | `crates/trpg-testing/src/principle_to_doc_trace.rs` | Add principle-to-doc trace module and matching contract test. | Verify principles point to current authoritative docs and S11 evidence. |
| `CODEX-0872-10-TESTING-QUALITY-37373a8f49` | `P0042.md` | supplemental-requirement | `testing_quality::requirement_to_test_trace` | Merge constraint into `requirement_to_test_trace`; no separate Rust output. | Covered by `requirement_to_test_trace_contract_tests`. |
| `CODEX-0873-10-TESTING-QUALITY-17111d90f9` | `P0043.md` | supplemental-requirement | `testing_quality::requirement_to_test_trace` | Merge constraint into `requirement_to_test_trace`; no separate Rust output. | Covered by `requirement_to_test_trace_contract_tests`. |
| `CODEX-0874-10-TESTING-QUALITY-95e0ac6e0d` | `P0044.md` | primary-implementation | `crates/trpg-testing/src/golden_scenarios_ci_impl.rs` | Add golden scenarios CI implementation module and matching contract test. | Verify tutorial/golden scenario fixtures, replay, export diff, and visibility gates. |
| `CODEX-0875-10-TESTING-QUALITY-a2e797e671` | `P0045.md` | primary-implementation | `crates/trpg-testing/src/test_strategy_impl.rs` | Add test strategy implementation module and matching contract test. | Verify minimal and stage command sets and negative case coverage. |
| `CODEX-0876-10-TESTING-QUALITY-ad4716763d` | `P0046.md` | documentation-or-traceability | `docs/codex/10-testing-quality/source_processing_record_docs_implementation_00_index_decision_trace_map.md` | Add source processing record only. | Markdown self-check plus batch evidence. |
| `CODEX-0877-10-TESTING-QUALITY-5a0fb801cc` | `P0049.md` | documentation-or-traceability | `docs/codex/10-testing-quality/source_processing_record_docs_checklists_implementation_acceptance_checklist.md` | Add source processing record only. | Markdown self-check plus batch evidence. |
| `CODEX-0878-10-TESTING-QUALITY-6d59753ce7` | `P0051.md` | documentation-or-traceability | `docs/codex/10-testing-quality/source_processing_record_docs_implementation_10_testing_quality_readme.md` | Add source processing record only. | Markdown self-check plus batch evidence. |
| `CODEX-0879-10-TESTING-QUALITY-b0eba279f4` | `P0054.md` | documentation-or-traceability | `docs/codex/10-testing-quality/source_processing_record_docs_implementation_10_testing_quality_contract_test_matrix.md` | Add source processing record only. | Markdown self-check plus batch evidence. |
| `CODEX-0880-10-TESTING-QUALITY-cc964ce88c` | `P0058.md` | documentation-or-traceability | `docs/codex/10-testing-quality/source_processing_record_docs_implementation_10_testing_quality_model_certification_tests.md` | Add source processing record only. | Markdown self-check plus batch evidence. |

## Checks

1. Minimal related check: `cargo test -p trpg-testing --all-features`
2. Stage checks:
   - `cargo test -p trpg-testing --test golden_scenarios_ci --all-features`
   - `cargo test -p trpg-testing --test visibility_leakage --all-features`
   - `cargo test -p trpg-testing --test model_certification_tests --all-features`
3. Formatting gate: `cargo fmt --all -- --check`
