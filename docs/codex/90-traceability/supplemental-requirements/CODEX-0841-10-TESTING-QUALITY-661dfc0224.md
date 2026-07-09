# Supplemental Requirement: CODEX-0841-10-TESTING-QUALITY-661dfc0224

Batch: `BATCH-038-10-testing-quality`  
Prompt file: `codex-prompts/10-testing-quality/P0011.md`  
Primary prompt: `CODEX-0089-10-TESTING-QUALITY-da28af3028`  
Current module: `testing_quality::benchmark_plan`

## Boundary

This prompt is supplemental only and does not own Rust output.

## Merge Instructions

- Require explicit benchmark thresholds for golden scenario, visibility/export diff, and model certification checks.
- Keep metric names under the current `testing_quality` namespace.

## Test Responsibility

Merged into `crates/trpg-testing/tests/benchmark_plan_contract_tests.rs`.
