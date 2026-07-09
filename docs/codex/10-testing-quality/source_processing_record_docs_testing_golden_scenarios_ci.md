# Source Processing Record: Testing Golden Scenarios CI

Prompt ID: `CODEX-0887-10-TESTING-QUALITY-c0507ea620`
Prompt file: `codex-prompts/10-testing-quality/P0056.md`
Role: `documentation-or-traceability`
Current-safe output: `docs/codex/10-testing-quality/source_processing_record_docs_testing_golden_scenarios_ci.md`

## Provenance Boundary

B040 source-processing names are audit-only. Current-safe implementation remains `testing_quality::testing_golden_scenarios_ci`.

## Current-safe Handling

- This Markdown record links the processed source to S11 golden scenario CI responsibility.
- It creates no code and no CI workflow.
- Scenario fixtures stay under current fixture paths.

## Governance Checks

- Golden scenario tests must cover Agent, Visibility, review, split-party play, secret dice, replay, and export assertions.
- Fixture-derived checks cannot leak keeper-only or private player content.
- Any future code change must be owned by a primary prompt.
