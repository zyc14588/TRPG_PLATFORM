# Supplemental Requirement: CODEX-0855-10-TESTING-QUALITY-0adc8f6280

Batch: `BATCH-038-10-testing-quality`  
Prompt file: `codex-prompts/10-testing-quality/P0019.md`  
Primary prompt: `CODEX-0094-10-TESTING-QUALITY-6ac95ec41f`  
Current module: `testing_quality::testing_golden_ci`

## Boundary

This prompt is supplemental only and does not own Rust output.

## Merge Instructions

- CI evidence must include the commands used for the minimal crate gate and S11-focused gates.
- Stage fixture expectations must remain stricter than batch-local smoke checks.

## Test Responsibility

Merged into `crates/trpg-testing/tests/testing_golden_ci_contract_tests.rs`.
