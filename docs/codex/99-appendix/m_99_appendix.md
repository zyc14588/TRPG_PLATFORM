# 99-appendix current-safe overview

> BATCH-051 current-safe documentation output. This page is an index and
> ownership trace; it does not turn appendix material into product scope.

## Current-safe metadata

| Prompt ID | Prompt file | Source file (provenance only) | Source SHA256 | Current crate | Current module | Current output |
|---|---|---|---|---|---|---|
| CODEX-1099-99-APPENDIX-e2d7df571d | codex-prompts/99-appendix/P0031.md | docs/implementation/99-appendix/README.md | 8ba5aa574b587e2a298e4de92ae342156eda65e9e1f5e67bf312010b60f4c38e | trpg-docs-governance | appendix_research::m_99_appendix | docs/codex/99-appendix/m_99_appendix.md |

The source path and hash above are provenance only. Current naming comes from
the normalized and current-safe maps.

## Allowed change boundary

- Maintain this Markdown index and BATCH-051 evidence only.
- Do not use appendix templates, terms, research notes, or historical source
  sketches to override current architecture or acceptance requirements.
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

## Appendix index

- [Directory scope](README.md) and
  [per-file prompt manifest](per-file-prompt-manifest.md)
- [Codex prompt template](codex-prompt-template.md)
- [Document template](document_template.md),
  [glossary](glossary.md), and
  [implementation document template](implementation_doc_template.md)
- [Follow-up research prompts](followup_research_prompts.md) and
  [ChatGPT follow-up research prompts](chatgpt_followup_research_prompts.md)
- [Open-source reference notes](open_source_reference_notes.md) and
  [unresolved questions](unresolved_questions.md)
- BATCH-051 provenance and dated records:
  [AI research provenance](chat_gpt.md),
  [previous follow-up prompts](followup_prompts_previous.md),
  [previous strict document template](docs_implementation_99_appendix_document_template_strict_previous.md),
  [previous strict glossary](docs_implementation_99_appendix_glossary_strict_previous.md),
  [strict implementation-document template](docs_implementation_99_appendix_implementation_doc_template_strict.md),
  [previous prototype catalog](prototype_catalog_previous-provenance.md),
  [previous open questions](open_questions_previous.md), and
  [dated research notes](research_notes_2026_06_30.md)

The manifest is an inventory; the normalized execution map, current-safe
module/output map, and token rewrite table remain the naming authority.

## Batch disposition and test responsibility

- Disposition: implemented as documentation-only traceability and index
  maintenance.
- BATCH-051 checks must verify both current maps, Prompt ID and provenance
  fields, crate/module/output agreement, Markdown structure, all relative
  links, governance invariants, and the docs-only boundary.
- S00 verification is the applicable stage check; this page owns no product
  test because BATCH-051 has no primary implementation prompt.
