# Previous Strict Document-Template Provenance

> This page is provenance only. It is not a current execution, implementation, template-authority, or acceptance entry point.

## Current-safe metadata

| Prompt ID | Prompt path | Source path | Source SHA256 | Crate | Current-safe module | Current-safe output |
|---|---|---|---|---|---|---|
| `CODEX-1075-99-APPENDIX-a3a2db4fc3` | `codex-prompts/99-appendix/P0007.md` | `docs/implementation/90-traceability/per-file-code-ready/99-appendix/docs-implementation-99-appendix-generated-from-source-strict-docs-implementation-99-appendix-document-template-e00a1e884b.v5-code-ready.md` | `53ecf777b88eca9d1eb7da5721876a5b4ea39c79a98bd4aa259ee46bbed5ee39` | `trpg-docs-governance` | `appendix_research::docs_implementation_99_appendix_document_template_previous_provenance` | `docs/codex/99-appendix/docs_implementation_99_appendix_document_template_strict_previous.md` |

The source path and hash are retained solely to trace the historical input.

## Allowed boundary

- Output role: `documentation-or-traceability`.
- This page may record the source identity, current-safe disposition, and review responsibility for P0007.
- It cannot supply an executable template, current product requirement, implementation plan, or acceptance result.
- Current construction uses the top-level design and normalized current-safe overlays instead.

## Governance red lines

- Authority Contracts remain immutable; authority changes require a fork.
- `HUMAN_KP` and `AI_KP` stay mutually exclusive.
- All AI use follows the Agent Gateway and governed runtime, never a direct business-layer provider call.
- AI cannot write canonical state, bypass rules, policy, or audit controls, fabricate dice, or expose restricted information.
- Formal changes follow `Command -> Workflow -> Decision -> Event Store -> Projection`; only the Event Store is canonical.
- Visibility labels and fact provenance remain end-to-end invariants.

## BATCH-051 disposition and test responsibility

- Disposition: preserve P0007 as non-executable provenance under the rewritten `previous_provenance` module and `strict_previous` output.
- Required checks: current-safe names match both normalized maps, the non-current warning is prominent, Markdown is valid, and no historical implementation proposal is copied.
- Test responsibility is documentation validation only; no code, migration, protocol, metric, workflow, or executable test is owned here.
