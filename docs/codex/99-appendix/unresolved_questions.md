# Unresolved questions

> BATCH-051 current-safe documentation output. An unresolved item remains open
> until current evidence and an authorized decision explicitly resolve it.

## Current-safe metadata

| Prompt ID | Prompt file | Source file (provenance only) | Source SHA256 | Current crate | Current module | Current output |
|---|---|---|---|---|---|---|
| CODEX-1080-99-APPENDIX-65f846f412 | codex-prompts/99-appendix/P0005.md | docs/implementation/90-traceability/per-file-code-ready/99-appendix/docs-implementation-99-appendix-generated-from-source-strict-docs-implementation-99-appendix-unresolved-questi-cff98994c1.v5-code-ready.md | 946747cd4271394a7e26eb4b5de3f812e1677c45852231723d352b5bf465cdbb | trpg-docs-governance | appendix_research::unresolved_questions | docs/codex/99-appendix/unresolved_questions.md |
| CODEX-1088-99-APPENDIX-7c8ccdfd7e | codex-prompts/99-appendix/P0017.md | docs/implementation/90-traceability/per-file-code-ready/99-appendix/docs-implementation-99-appendix-unresolved-questions-169711fa36.v5-code-ready.md | 355c7b2d93ebe635136a7645d9584c2799c9db768499783c951fe0fe8a9ddde2 | trpg-docs-governance | appendix_research::unresolved_questions | docs/codex/99-appendix/unresolved_questions.md |
| CODEX-1094-99-APPENDIX-368d7c729a | codex-prompts/99-appendix/P0023.md | docs/implementation/90-traceability/per-file-code-ready/99-appendix/sources-v3-baseline-document-group-docs-implementation-99-appendix-unresolved-questions-febf28b6fe.v5-code-ready.md | 066a7cfcffb90a8f59e25905a49009b4e07c97e32ba17e35cbb25ef619dc684f | trpg-docs-governance | appendix_research::unresolved_questions | docs/codex/99-appendix/unresolved_questions.md |

Source paths and hashes above are provenance only. Current naming comes from
the normalized and current-safe maps.

## Allowed change boundary

- Maintain this Markdown question trace and BATCH-051 evidence only.
- Do not guess a resolution, infer PASS from missing evidence, or convert an
  open question into product scope.
- Do not create or modify Rust source or tests, migrations, API or event
  contracts, messaging subjects, metrics, workflows, or formal state writes.

## Governance invariants retained

- Authority Contract remains immutable and fork-only; HUMAN_KP and AI_KP
  remain campaign-level mutually exclusive modes.
- AI access remains Agent Gateway to Agent Orchestrator/Runtime to Model
  Provider Adapter; AI cannot write the database or formal state directly.
- Formal rulings must pass tools, rules, state services, and the event log
  through Command to Workflow to Decision to Event Store to Projection.
- Event Store remains canonical; projections, caches, RAG indexes, and
  summaries remain rebuildable read models.
- Visibility Label, Fact Provenance, Tool Grant, Policy Gate, OpenFGA, OPA,
  and Audit Log protections cannot be bypassed.

## Question ledger rules

- The working list remains
  [Unresolved Codex Questions](unresolved-codex-questions.md).
- Every item needs an owner, status, required evidence, visibility, and fact
  provenance.
- Use open when evidence is absent and blocked only when the blocker is
  recorded. Use resolved only with an explicit current decision and evidence
  link; none is implied by this page.
- Questions that would change scope or authority require user direction and a
  separately authorized task.

## Batch disposition and test responsibility

- Disposition: implemented as documentation-only traceability; no unresolved
  question is marked PASS or resolved by BATCH-051.
- All three prompt rows share this single target and must remain present.
- BATCH-051 checks must verify map agreement, all Prompt IDs and provenance
  fields, Markdown structure, relative links, open-status semantics,
  governance invariants, and the docs-only boundary.
- S00 verification is the applicable stage check; this page owns no product
  test because BATCH-051 has no primary implementation prompt.
