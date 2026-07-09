# Supplemental Requirement: CODEX-0849-10-TESTING-QUALITY-3b745596ac

Batch: `BATCH-038-10-testing-quality`  
Prompt file: `codex-prompts/10-testing-quality/P0021.md`  
Primary prompt: `CODEX-0089-10-TESTING-QUALITY-da28af3028`  
Current module: `testing_quality::benchmark_plan`

## Boundary

This prompt is supplemental only and does not own Rust output.

## Merge Instructions

- Keep benchmark budgets explicit and deterministic.
- Treat projections, exports, summaries, and reports as rebuildable read models.

## Test Responsibility

Merged into `crates/trpg-testing/tests/benchmark_plan_contract_tests.rs`.
