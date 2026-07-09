# Supplemental Requirement: CODEX-0854-10-TESTING-QUALITY-705d02fdf8

Batch: `BATCH-038-10-testing-quality`  
Prompt file: `codex-prompts/10-testing-quality/P0022.md`  
Primary prompt: `CODEX-0093-10-TESTING-QUALITY-97f7f731a8`  
Current module: `testing_quality::test_strategy`

## Boundary

This prompt is supplemental only and does not own Rust output.

## Merge Instructions

- Require fixture-backed assertions for golden scenario, visibility leakage, export diff, and model eval.
- Keep acceptance failure repair rules intact.

## Test Responsibility

Merged into `crates/trpg-testing/tests/test_strategy_contract_tests.rs`.
