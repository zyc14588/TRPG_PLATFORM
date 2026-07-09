# Source Processing Record: Decision Trace Map

Prompt ID: `CODEX-0876-10-TESTING-QUALITY-ad4716763d`
Prompt file: `codex-prompts/10-testing-quality/P0046.md`
Role: `documentation-or-traceability`
Current-safe output: `docs/codex/10-testing-quality/source_processing_record_docs_implementation_00_index_decision_trace_map.md`

## Provenance Boundary

The source document name from B039 is retained only as provenance. Historical version and hash fragments are not current module, event, metric, workflow, migration, or test names.

## Current-safe Handling

- The active trace owner remains `testing_quality::decision_trace_map`.
- This record adds no Rust module, API route, migration, NATS subject, or Event Store schema.
- Governance coverage is verified through `crates/trpg-testing/tests/decision_trace_map_contract_tests.rs` and the B039 batch evidence.

## Governance Checks

- Formal decisions must travel through Command, Workflow, Decision, Event Store, and Projection.
- Visibility labels and fact provenance remain required command fields.
- AI output is not accepted as a direct state write.
