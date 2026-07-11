# requirement to test trace

> BATCH-050 current-safe traceability output. This page documents ownership and
> test responsibility only; it does not own a Rust module, migration, API,
> event, NATS subject, metric, workflow, or formal state-write path.

## Current-safe target

| Prompt ID | Prompt file | Current crate | Current module | Current output | Source file (provenance only) | Source SHA256 |
|---|---|---|---|---|---|---|
| `CODEX-1097-90-TRACEABILITY-15a234659d` | `codex-prompts/90-traceability/P0109.md` | `trpg-docs-governance` | `traceability::requirement_to_test_trace` | `docs/codex/90-traceability/requirement_to_test_trace.md` | `docs/implementation/90-traceability/requirement-to-test-trace.md` | `6a6694fe0c478400b942b3cb8e45f89b65ab80077c21739ed12874b110cd516d` |

## Allowed change boundary

- Maintain the current-safe Markdown trace and batch evidence only.
- Do not activate historical Rust, SQL, API, event, NATS, metric, or test-name
  proposals embedded in the provenance source.
- Do not create or modify business code or product tests: BATCH-050 contains no
  primary implementation prompt.

## Governance invariants retained

- Authority Contract remains immutable and fork-only; HUMAN_KP and AI_KP
  remain campaign-level mutually exclusive modes.
- AI capabilities route through Agent Gateway, Agent Orchestrator/Runtime, and
  Model Provider Adapter; AI does not write formal state directly.
- Formal decisions pass tools, rules, state services, and the event log through
  `Command -> Workflow -> Decision -> Event Store -> Projection`.
- Visibility Label and Fact Provenance remain mandatory across API, events,
  agent context, tool results, RAG, summaries, exports, replay, logs, and
  metrics.

## Batch disposition and test responsibility

- Disposition: docs-only traceability; the historical source's implementation
  sketches remain provenance and are not current output.
- Existing executable assertions are owned by
  `testing_quality::requirement_to_test_trace`, not this documentation prompt.
- Targeted check:
  `cargo test -p trpg-testing --test requirement_to_test_trace_contract_tests --all-features`.
- BATCH-050 must also verify Prompt ID/source SHA/current-safe map agreement,
  Markdown structure, referenced paths, and the docs-only boundary.
