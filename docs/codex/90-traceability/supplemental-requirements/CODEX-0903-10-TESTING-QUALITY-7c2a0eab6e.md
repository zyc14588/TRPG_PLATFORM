# Supplemental Requirement: Golden Scenarios CI Implementation

Prompt ID: `CODEX-0903-10-TESTING-QUALITY-7c2a0eab6e`
Primary Prompt: `CODEX-0874-10-TESTING-QUALITY-95e0ac6e0d`
Shared module: `testing_quality::golden_scenarios_ci_impl`
Prompt file: `codex-prompts/10-testing-quality/P0074.md`

## Merge Instructions

- Golden scenarios CI must preserve event replay, split-party privacy, secret dice, and export assertions.
- Fixture checks must not leak private or keeper-only content.
- This supplemental prompt owns no Rust src/test output.
