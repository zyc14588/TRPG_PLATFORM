# Historical research decision matrix provenance

> BATCH-052 current-safe documentation output. This page records earlier
> research lineage only; it is not a current technology decision, version
> recommendation, implementation plan, or acceptance entry.

## Current-safe metadata

| Prompt ID | Prompt file | Source file (provenance only) | Source SHA256 | Current crate | Current module | Current output |
|---|---|---|---|---|---|---|
| `CODEX-1106-99-APPENDIX-ef131affa7` | `codex-prompts/99-appendix/P0032.md` | `docs/implementation/99-appendix/research-decision-matrix-v5.md` | `41febf22011632cd863c6b2c96edb9492d7826e12940a697661b000b98cce959` | `trpg-docs-governance` | `appendix_research::research_decision_matrix_previous_provenance` | `docs/codex/99-appendix/research_decision_matrix_previous.md` |

The source path, historical version, links, and technology statements are
provenance only. This batch does not refresh external research or infer that
an earlier option remains current, supported, secure, or accepted.

## Disposition

- Do not turn the historical comparison into dependencies, adapters,
  migrations, contracts, messaging, metrics, workflows, tests, or release
  gates.
- Current technology decisions remain governed by higher-priority repository
  design and the owning implementation stage.
- Any renewed comparison requires a bounded, separately authorized research
  task using current primary sources and explicit decision evidence.
- This page cannot be cited as stage PASS, V1 PASS, or release approval.

## Governance boundary

- Authority Contract remains immutable and fork-only; `HUMAN_KP` and `AI_KP`
  remain campaign-level mutually exclusive.
- AI capability remains behind Agent Gateway, Orchestrator/Runtime, and Model
  Provider Adapter; AI cannot write the database or formal state directly.
- Formal rulings remain governed by tools, rules, state services, and the
  Event Store; projections and RAG indexes are rebuildable read models.
- Visibility Label, Fact Provenance, Tool Permission Gate, Policy Gate, and
  audit controls remain mandatory regardless of a technology choice.

## BATCH-052 disposition and test responsibility

P0032 is implemented as previous/provenance documentation only. B052 verifies
exact map metadata, the non-current decision boundary, Markdown structure,
retained governance invariants, and absence of executable implementation
output. No product test is owned because the batch has no primary prompt.
