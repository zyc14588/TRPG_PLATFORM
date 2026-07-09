# Source Processing Record: Traceability Golden Scenarios CI Breakdown

Prompt ID: `CODEX-0886-10-TESTING-QUALITY-16dc13a1c3`
Prompt file: `codex-prompts/10-testing-quality/P0047.md`
Role: `documentation-or-traceability`
Current-safe output: `docs/codex/10-testing-quality/source_processing_record_docs_implementation_90_traceability_source_breakdown_testing_golden_scenarios_ci.md`

## Provenance Boundary

The historical source-processing title is provenance only. Current execution must use `testing_quality::golden_scenarios_ci_impl` for implementation ownership.

## Current-safe Handling

- This record documents B040 coverage of the golden scenarios CI trace.
- No Rust module, migration, API route, workflow, event schema, or NATS subject is created here.
- Golden scenario implementation constraints stay with the current primary module.

## Governance Checks

- Golden scenario CI must verify Event Store replay, server-side dice, visibility redaction, split-party privacy, and export boundaries.
- Agent output cannot bypass Tool Permission Gate or Decision Commit Pipeline.
- Current evidence must not depend on source-archive execution.
