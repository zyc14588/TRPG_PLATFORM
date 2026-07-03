# COC AI TRPG Top Level Design Trace

> Prompt ID: CODEX-0145-00-INDEX-72bf3e1f4d
> Role: documentation-or-traceability
> Current module: docs_governance::coc_ai_trpg_top_level_design
> Current output: `docs/codex/00-index/coc_ai_trpg_top_level_design.md`

This trace points Batch-002 back to the authoritative top-level design:
`docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md`.

## Protected Red Lines

- V1 is a complete playable COC 7 loop, not a lightweight demo.
- HUMAN_KP and AI_KP are campaign-level mutually exclusive authority modes.
- Authority Contract is immutable after creation except by fork.
- Formal state writes must flow through Command, Workflow, Decision, Event
  Store, and Projection.
- Event Store is authoritative; projections, caches, RAG indexes, and summaries
  are rebuildable read models.
- Visibility Label and Fact Provenance must flow through APIs, events, agent
  context, tool results, RAG, summaries, export, replay, logs, and metrics.
- Formal dice are server generated and recorded.

## Batch-002 Interpretation

This file records constraints only. It does not implement the top-level design.
