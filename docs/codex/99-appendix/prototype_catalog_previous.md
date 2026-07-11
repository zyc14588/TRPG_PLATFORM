# Historical prototype catalog provenance

> BATCH-052 current-safe documentation output. This page is read-only
> provenance, not a current implementation plan, backlog, test catalog, stage
> acceptance entry, or V1 acceptance entry.

## Current-safe metadata

| Prompt ID | Prompt file | Source file (provenance only) | Source SHA256 | Current crate | Current module | Current output |
|---|---|---|---|---|---|---|
| `CODEX-1104-99-APPENDIX-645d245a1f` | `codex-prompts/99-appendix/P0029.md` | `docs/implementation/99-appendix/minimal-rust-prototype-catalog-v5.md` | `6cf144db58160d378195a834f1a78a2a94f3859c63f9b6fc23e3639e9359ee1e` | `trpg-docs-governance` | `appendix_research::prototype_catalog_previous_provenance` | `docs/codex/99-appendix/prototype_catalog_previous.md` |

The source path, version fragment, hash-derived names, and historical test
names are provenance only. Current engineering names come exclusively from
the normalized and current-safe maps.

## Disposition

- Do not copy, schedule, run, or claim completion of the historical prototype
  rows.
- Historical crate, module, output, migration, event, metric, subject, and test
  suggestions do not create current work.
- A still-useful requirement must be raised by an authorized current stage or
  batch, mapped to a current-safe primary prompt, and given fresh tests and
  acceptance evidence.
- This page owns Markdown traceability only and creates no Rust source/test,
  dependency, schema, API, workflow, provider call, or formal state write.

## Governance boundary

- Authority Contract remains immutable and fork-only; `HUMAN_KP` and `AI_KP`
  remain campaign-level mutually exclusive.
- AI capability remains behind Agent Gateway, Orchestrator/Runtime, and Model
  Provider Adapter; AI cannot write the database or formal state directly.
- Formal rulings remain governed by tools, rules, state services, and the
  Event Store; projections and RAG indexes are rebuildable read models.
- Visibility Label, Fact Provenance, Tool Permission Gate, Policy Gate, and
  audit controls cannot be bypassed for a prototype.

## BATCH-052 disposition and test responsibility

P0029 is implemented as previous/provenance documentation only. B052 verifies
exact map metadata, the non-current boundary, Markdown structure, retained
governance invariants, and absence of executable implementation output. No
product test is owned because the batch has no primary prompt.
