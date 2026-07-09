# BATCH-041 Work Plan

Batch: `BATCH-041-10-testing-quality`
Stage: `S11-testing-quality-golden-ci`
Declared prompts: 3
Primary prompts found by current-safe maps: 1 (`CODEX-0906-10-TESTING-QUALITY-d70cab3757`)

## Scope Rule

Only B041 outputs are in scope. `source-archive/**` and historical V3/V4/V5/V6 path fragments remain provenance only and are not used for current module, migration, event, metric, workflow, test, or output names.

## Prompt Map

| Prompt ID | Prompt | Role | Current-safe target | Allowed change | Test responsibility |
| --- | --- | --- | --- | --- | --- |
| `CODEX-0906-10-TESTING-QUALITY-d70cab3757` | `P0076.md` | primary-implementation | `crates/trpg-testing/src/golden_scenarios_ci.rs` and `crates/trpg-testing/tests/golden_scenarios_ci_contract_tests.rs` | Current-safe Rust test harness module and contract test only; register in `trpg-testing` shared contract list | `golden_scenarios_ci_contract_tests`; `golden_scenarios_ci`; `trpg-testing` crate tests |
| `CODEX-0907-10-TESTING-QUALITY-86a266c57b` | `P0077.md` | supplemental-requirement | `codex-prompts/10-testing-quality/P0077.md` | Supplemental Markdown only; no Rust output | Boundary check: primary target remains `CODEX-0093-10-TESTING-QUALITY-97f7f731a8` |
| `CODEX-0908-10-TESTING-QUALITY-3b88dc5203` | `P0078.md` | supplemental-requirement | `codex-prompts/10-testing-quality/P0078.md` | Supplemental Markdown only; no Rust output | Boundary check: primary target remains `CODEX-0892-10-TESTING-QUALITY-1b68a77fb7` |

## Planned Checks

1. Minimal related: `cargo test -p trpg-testing --test golden_scenarios_ci_contract_tests --all-features`
2. Minimal related: `cargo test -p trpg-testing --test golden_scenarios_ci --all-features`
3. Crate scope: `cargo test -p trpg-testing --all-features`
4. Stage checks: `cargo test -p trpg-testing --test visibility_leakage --all-features`; `cargo test -p trpg-testing --test model_certification_tests --all-features`
5. Formatting: `cargo fmt --all -- --check`

## Risk

The user-supplied batch fact said primary prompt count was 0, while `batches/B041.md`, `CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`, `CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md`, and `per-file-prompt-manifest.md` identify one B041 primary prompt. This run follows the normalized current-safe mapping and records the discrepancy here.
