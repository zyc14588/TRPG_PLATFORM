# Open-source reference notes

> BATCH-051/BATCH-052 current-safe documentation output. References collected here are
> advisory inputs only and never override the current top-level design.

## Current-safe metadata

| Prompt ID | Prompt file | Source file (provenance only) | Source SHA256 | Current crate | Current module | Current output |
|---|---|---|---|---|---|---|
| CODEX-1079-99-APPENDIX-8cfa2d0624 | codex-prompts/99-appendix/P0006.md | docs/implementation/90-traceability/per-file-code-ready/99-appendix/docs-implementation-99-appendix-generated-from-source-strict-docs-implementation-99-appendix-open-source-refer-224a23681e.v5-code-ready.md | dd3dde91fc89ad9bfb014115b4010be73537b8eadff5dbd980d74435c4eef29e | trpg-docs-governance | appendix_research::open_source_reference_notes | docs/codex/99-appendix/open_source_reference_notes.md |
| CODEX-1086-99-APPENDIX-678a4fceb9 | codex-prompts/99-appendix/P0015.md | docs/implementation/90-traceability/per-file-code-ready/99-appendix/docs-implementation-99-appendix-open-source-reference-notes-d7843d5f9a.v5-code-ready.md | cbd505a58fb0b7c6ce57270d8a0b03bf0796607c86e98d955c92fc821c878e90 | trpg-docs-governance | appendix_research::open_source_reference_notes | docs/codex/99-appendix/open_source_reference_notes.md |
| CODEX-1093-99-APPENDIX-e48b12230c | codex-prompts/99-appendix/P0022.md | docs/implementation/90-traceability/per-file-code-ready/99-appendix/sources-v3-baseline-document-group-docs-implementation-99-appendix-open-source-reference-notes-8a1c838908.v5-code-ready.md | 8bae0fc0ca79a06ceb0951b4fc64d2d3a9c6230ba9c2ffc3f984a53d384b9d14 | trpg-docs-governance | appendix_research::open_source_reference_notes | docs/codex/99-appendix/open_source_reference_notes.md |
| CODEX-1105-99-APPENDIX-853faddcb9 | codex-prompts/99-appendix/P0030.md | docs/implementation/99-appendix/open-source-reference-notes.md | 8a08ef712b577950692eff711b1ef2567dedfb3f9c99ea735178cb3b859871b7 | trpg-docs-governance | appendix_research::open_source_reference_notes | docs/codex/99-appendix/open_source_reference_notes.md |

Source paths and hashes above are provenance only. Current naming comes from
the normalized and current-safe maps.

## Allowed change boundary

- Maintain this Markdown reference trace and BATCH-051/BATCH-052 evidence only.
- Do not infer current versions, licenses, security posture, compatibility, or
  adoption decisions without separately gathered and cited evidence.
- Do not promote reference material into Rust source or tests, migrations,
  contracts, messaging, metrics, workflows, or formal state-write paths.

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

## Reference index

- [Codex official reference summary](codex-official-reference-notes.md)
  records the repository's existing official Codex links.
- [Current normalized prompt execution map](../00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md)
  and [current-safe module/output map](../00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md)
  control executable naming.
- This batch performs no external research. Any later evaluation must verify
  claims against current authoritative sources and record provenance.

## Batch disposition and test responsibility

- Disposition: implemented as documentation-only traceability.
- All four prompt rows share this single target and must remain present.
- BATCH-051 and BATCH-052 checks must verify map agreement, their Prompt IDs and provenance
  fields, Markdown structure, relative links, the advisory-only status of
  references, governance invariants, and the docs-only boundary.
- S00 verification is the applicable stage check; this page owns no product
  test because neither batch has a primary implementation prompt.
