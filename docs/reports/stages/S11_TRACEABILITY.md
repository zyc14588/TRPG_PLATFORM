# S11 Traceability

Stage: `S11-testing-quality-golden-ci`
Batch evidence repaired: `BATCH-041-10-testing-quality`
Run date: 2026-07-09

## Inputs Re-read

- `AGENTS.md`
- `batches/B041.md`
- `codex-prompts/10-testing-quality/P0076.md`
- `codex-prompts/10-testing-quality/P0077.md`
- `codex-prompts/10-testing-quality/P0078.md`
- `stages/s11-testing-quality-golden-ci/TEST_PLAN.md`
- `stages/s11-testing-quality-golden-ci/TEST_DATA.md`
- `stages/s11-testing-quality-golden-ci/ACCEPTANCE_PROMPT.md`
- `fixtures/stages/S11_stage_acceptance_fixture.v1.json.md`
- `fixtures/stages/detailed/S11_golden_visibility_export_diff_expected.current.json.md`
- `docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`
- `docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md`
- `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`

## B041 Prompt Trace

| Prompt ID | Prompt file | Role | Current-safe crate/module | Current-safe output | Test/evidence |
| --- | --- | --- | --- | --- | --- |
| `CODEX-0906-10-TESTING-QUALITY-d70cab3757` | `codex-prompts/10-testing-quality/P0076.md` | primary-implementation | `trpg-testing`; `testing_quality::golden_scenarios_ci` | `crates/trpg-testing/src/golden_scenarios_ci.rs`; `crates/trpg-testing/tests/golden_scenarios_ci_contract_tests.rs` | `golden_scenarios_ci_contract_tests`, `golden_scenarios_ci`, B041 acceptance evidence |
| `CODEX-0907-10-TESTING-QUALITY-86a266c57b` | `codex-prompts/10-testing-quality/P0077.md` | supplemental-requirement | `trpg-testing`; `testing_quality::test_strategy` | `codex-prompts/10-testing-quality/P0077.md` | Supplemental boundary evidence; no Rust output |
| `CODEX-0908-10-TESTING-QUALITY-3b88dc5203` | `codex-prompts/10-testing-quality/P0078.md` | supplemental-requirement | `trpg-testing`; `testing_quality::latest_deep_research_rust_summary` | `codex-prompts/10-testing-quality/P0078.md` | Supplemental boundary evidence; no Rust output |

## Fixture Trace

| Fixture requirement | Covered by |
| --- | --- |
| `docs/reports/stages/S11_ACCEPTANCE_EVIDENCE.md` | Stage acceptance report created in this repair. |
| `docs/reports/stages/S11_TEST_RESULTS.md` | Stage test result report created in this repair. |
| `docs/reports/stages/S11_TRACEABILITY.md` | This file. |
| `evidence/stages/S11/golden-scenario.txt` | Golden scenario fixture evidence. |
| `evidence/stages/S11/visibility-leakage.txt` | Visibility leakage fixture evidence. |
| `evidence/stages/S11/export-diff.txt` | Export diff fixture evidence. |

## Expected Record / Error Trace

| Fixture item | Automated assertion / evidence |
| --- | --- |
| `ScenarioTestReport.steps` | `golden_scenarios_ci_contract_tests` checks golden action markers and `golden_scenarios_ci` stage gate. |
| `ScenarioTestReport.dice` | `golden_scenarios_ci` checks server dice source marker. |
| `ScenarioTestReport.decisions` | `golden_scenarios_ci` checks Command -> Workflow -> Decision -> EventStore -> Projection path. |
| `ScenarioTestReport.final_state_hash` | Represented by replay/golden stage evidence and `trpg-testing` replay consistency tests. |
| `ExportDiffReport.player_export_hash` | Export diff evidence asserts player export excludes restricted markers. |
| `ExportDiffReport.keeper_export_hash` | Export diff evidence records separate keeper export boundary. |
| `ExportDiffReport.audit_export_hash` | Export diff evidence records audit export must contain dice, decisions, model route snapshot. |
| `ExportDiffReport.redacted_fields` | `golden_scenarios_ci_contract_tests` and `visibility_leakage` verify redaction markers. |
| `VISIBILITY_LEAKAGE_DETECTED` | Covered by `golden_scenarios_ci_contract_tests` and `visibility_leakage`. |
| `GOLDEN_SCENARIO_RULE_VIOLATION` | Covered by `golden_scenarios_ci_contract_tests`. |
| `KEEPER_SECRET_REVEALED` | Covered by `golden_scenarios_ci_contract_tests` failure-case marker. |

## Boundary Trace

- Authority Contract immutable / KP mutual exclusion: checked through direct B041 authority negative test and workspace authority tests.
- Agent Gateway-only model access: no evidence repair introduced provider calls; workspace tests for agent runtime/provider boundaries passed.
- Tool Permission Gate / Policy Gate: covered by workspace agent/runtime tests and S11 model certification gates.
- Event Store canon: B041 contract records governed events; workspace event store tests passed.
- Visibility Label / Fact Provenance: B041 and S11 visibility tests passed.
- No supplemental scope expansion: P0077 and P0078 remain Markdown-only supplemental requirements.

## Non-applicable Trace

- pnpm: no package manifest or lockfile.
- Docker: no Docker change in B041 evidence repair; deployment smoke is S09/S13 trace.
- SQLx/OpenAPI/NATS/WebSocket schema generation: no schema or migration changes in this repair.
