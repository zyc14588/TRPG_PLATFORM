# ChatGPT follow-up research prompts

> BATCH-051 current-safe documentation output. This page records a governed
> research handoff and does not send a request to any model or external
> service.

## Current-safe metadata

| Prompt ID | Prompt file | Source file (provenance only) | Source SHA256 | Current crate | Current module | Current output |
|---|---|---|---|---|---|---|
| CODEX-1095-99-APPENDIX-499db16dee | codex-prompts/99-appendix/P0024.md | docs/implementation/90-traceability/per-file-code-ready/99-appendix/sources-v3-baseline-document-group-docs-prompts-chatgpt-followup-research-prompts-0e08a2e0b2.v5-code-ready.md | 8310e16a318e08bd380a03b85c86d29c6fedd8f65dae0dc784ce20561184f657 | trpg-docs-governance | appendix_research::chatgpt_followup_research_prompts | docs/codex/99-appendix/chatgpt_followup_research_prompts.md |

The source path and hash above are provenance only. Current naming comes from
the normalized and current-safe maps.

## Allowed change boundary

- Maintain this Markdown handoff and BATCH-051 evidence only.
- Do not perform external research or invoke a model directly. A future
  authorized AI task must use the Agent Gateway path and its policy controls.
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

## Minimum handoff fields

- Link the governed question from
  [Follow-up research prompts](followup_research_prompts.md).
- State the requested evidence, permitted inputs, visibility, fact provenance,
  owner, and completion criterion.
- Require a docs-only result and an explicit unresolved status when evidence
  is insufficient.
- Treat model output as a proposal requiring validation, never as a formal
  ruling or state mutation.

## Batch disposition and test responsibility

- Disposition: implemented as documentation-only traceability; no research or
  model call was executed.
- BATCH-051 checks must verify both current maps, Prompt ID and provenance
  fields, crate/module/output agreement, Markdown structure, relative links,
  Gateway-only AI access, governance invariants, and the docs-only boundary.
- S00 verification is the applicable stage check; this page owns no product
  test because BATCH-051 has no primary implementation prompt.
