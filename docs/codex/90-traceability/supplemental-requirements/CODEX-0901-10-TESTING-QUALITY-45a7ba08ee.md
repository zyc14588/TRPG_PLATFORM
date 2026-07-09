# Supplemental Requirement: Testing Golden CI

Prompt ID: `CODEX-0901-10-TESTING-QUALITY-45a7ba08ee`
Primary Prompt: `CODEX-0094-10-TESTING-QUALITY-6ac95ec41f`
Shared module: `testing_quality::testing_golden_ci`
Prompt file: `codex-prompts/10-testing-quality/P0070.md`

## Merge Instructions

- Golden CI must include replay, visibility, export diff, and provider certification gates.
- Failure repair cannot delete tests or weaken policy gates.
- This supplemental prompt owns no Rust src/test output.
