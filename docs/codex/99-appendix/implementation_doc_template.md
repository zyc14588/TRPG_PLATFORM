# Current implementation-document template

> BATCH-051 current-safe documentation output. This is a template for
> describing authorized work, not authorization to implement it.

## Current-safe ownership

| Prompt ID | Prompt file | Current crate | Current module | Current output | Source file (provenance only) | Source SHA256 |
|---|---|---|---|---|---|---|
| `CODEX-1083-99-APPENDIX-cd4ea494b1` | `codex-prompts/99-appendix/P0012.md` | `trpg-docs-governance` | `appendix_research::implementation_doc_template` | `docs/codex/99-appendix/implementation_doc_template.md` | `docs/implementation/90-traceability/per-file-code-ready/99-appendix/docs-implementation-99-appendix-implementation-doc-template-c78a6ad159.v5-code-ready.md` | `bd7a87c13b5a00ccdf576a8bd18a068035bb3c233f7867213d5c54446964ab93` |
| `CODEX-1092-99-APPENDIX-63da50377b` | `codex-prompts/99-appendix/P0021.md` | `trpg-docs-governance` | `appendix_research::implementation_doc_template` | `docs/codex/99-appendix/implementation_doc_template.md` | `docs/implementation/90-traceability/per-file-code-ready/99-appendix/sources-v3-baseline-document-group-docs-implementation-99-appendix-implementation-doc-template-c8c2d42b0d.v5-code-ready.md` | `bcca1b1b4044b5028b7d96f91c7d20746640a85b21e1977adbfc3158a60e2a1b` |

## Required sections for an implementation document

1. Prompt IDs, output roles, current-safe targets, and owning stage/batch.
2. Scope, explicit exclusions, affected components, and trust boundaries.
3. Authority Contract immutability, `HUMAN_KP` / `AI_KP` mutual exclusion,
   Event Store canon, Visibility, Fact Provenance, Agent Gateway, and Policy
   Gate invariants relevant to the change.
4. Existing code paths to reuse and the minimum authorized change.
5. Contract, migration, observability, and compatibility impact, or explicit
   N/A reasons.
6. Positive and negative tests, exact commands, fixtures, and evidence paths.
7. Rollback or repair boundary, unresolved risks, and next-batch handoff.

The owning primary prompt must authorize any concrete implementation. A
documentation role may fill this template only as requirements or
traceability and cannot create product code by implication.

## Test responsibility

B051 checks the two owning rows, current-safe metadata, shared-target merge,
Markdown structure, and the explicit no-code authorization boundary.
