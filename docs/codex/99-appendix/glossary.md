# Current governance glossary

> BATCH-051/BATCH-052 current-safe documentation output. Definitions defer to the current
> top-level design and repository authority order.

## Current-safe ownership

| Prompt ID | Prompt file | Current crate | Current module | Current output | Source file (provenance only) | Source SHA256 |
|---|---|---|---|---|---|---|
| `CODEX-1082-99-APPENDIX-fa5d472e21` | `codex-prompts/99-appendix/P0011.md` | `trpg-docs-governance` | `appendix_research::glossary` | `docs/codex/99-appendix/glossary.md` | `docs/implementation/90-traceability/per-file-code-ready/99-appendix/docs-implementation-99-appendix-glossary-4560f57299.v5-code-ready.md` | `bf97546107fb4fc04efaa717b752f64dc76fba44853ed06fae3a04ed0ec4c5bc` |
| `CODEX-1091-99-APPENDIX-6fac5c6fee` | `codex-prompts/99-appendix/P0020.md` | `trpg-docs-governance` | `appendix_research::glossary` | `docs/codex/99-appendix/glossary.md` | `docs/implementation/90-traceability/per-file-code-ready/99-appendix/sources-v3-baseline-document-group-docs-implementation-99-appendix-glossary-2f0c468494.v5-code-ready.md` | `27d24b56cd828f1ab24e47245881576b9cf12398df05e9d61e7016d898960e1a` |
| `CODEX-1102-99-APPENDIX-6512e2632c` | `codex-prompts/99-appendix/P0027.md` | `trpg-docs-governance` | `appendix_research::glossary` | `docs/codex/99-appendix/glossary.md` | `docs/implementation/99-appendix/glossary.md` | `9b84e64d868384a174105cba53a1ec0e7acf4925fc011ec288bb6be9014bcf3c` |

## Terms

| Term | Current meaning |
|---|---|
| Authority Contract | Immutable Campaign-level snapshot of authority mode, owner, rules, prompts, agent pack, tools, safety, and model route; changes require a fork. |
| HUMAN_KP | Campaign mode in which the human Keeper is final authority and AI output is draft-only with human confirmation. |
| AI_KP | Campaign mode in which the AI Keeper Orchestrator may request formal decisions through approved tools; humans cannot override history in place. |
| Proposal / ToolCall / DraftDecision | Non-canonical AI outputs that require governed validation and commit before becoming formal state. |
| Decision Commit Pipeline | The governed route from command and workflow decision to Event Store append and rebuildable projections. |
| Event Store | The canonical history of formal game facts; projections, caches, RAG indexes, and summaries are rebuildable read models. |
| Visibility Label | Access label that constrains data and every derived output; derivation may not widen visibility. |
| Fact Provenance | Source and status carried with a fact, distinguishing proposed, claimed, confirmed, contradicted, and other states. |
| Tool Permission Gate | Default-deny authorization gate for Agent tool requests, including authority, permission, state, ruleset, visibility, schema, and safety checks. |
| Policy Gate | Default-deny policy enforcement that Agents, plugins, providers, and handlers cannot bypass. |
| Agent Gateway | The mandatory business entry point for AI capability before orchestration/runtime and a model provider adapter. |
| Idempotency key | Command identity used to prevent duplicate formal effects when a request is retried. |
| Expected version | Optimistic-concurrency expectation that rejects a write against a changed aggregate stream. |
| Previous provenance | Read-only evidence of an earlier input or decision lineage; never a current implementation or acceptance entry. |

## Boundary and test responsibility

The glossary defines terms; it does not define new product scope. B051 and
B052 check all three owners, source metadata, map agreement, table shape,
forbidden historical current naming, and retained governance semantics.
