# Previous Strict Glossary Provenance

> This page is provenance only. It is not a current execution, terminology-authority, implementation, or acceptance entry point.

## Current-safe metadata

| Prompt ID | Prompt path | Source path | Source SHA256 | Crate | Current-safe module | Current-safe output |
|---|---|---|---|---|---|---|
| `CODEX-1077-99-APPENDIX-dde42583c6` | `codex-prompts/99-appendix/P0008.md` | `docs/implementation/90-traceability/per-file-code-ready/99-appendix/docs-implementation-99-appendix-generated-from-source-strict-docs-implementation-99-appendix-glossary-strict-v-b8f29a8d23.v5-code-ready.md` | `178fa188cbed45d38294ccd0834a04919277e4251ba7aafa107a53efb0c32d0a` | `trpg-docs-governance` | `appendix_research::docs_implementation_99_appendix_glossary_previous_provenance` | `docs/codex/99-appendix/docs_implementation_99_appendix_glossary_strict_previous.md` |

The source path and hash are retained solely to trace the historical input.

## Allowed boundary

- Output role: `documentation-or-traceability`.
- This page may record source identity, current-safe disposition, and review responsibility for P0008.
- Historical glossary wording cannot redefine current domain terms, engineering names, scope, or acceptance criteria.
- Current terminology follows the top-level design and normalized current-safe overlays.

## Governance red lines

- Authority Contracts remain immutable and authority changes are fork-only.
- `HUMAN_KP` and `AI_KP` remain mutually exclusive campaign modes.
- AI calls use the Agent Gateway and governed runtime; business code never calls providers directly.
- AI cannot write canonical state, bypass rules, state services, policy, or audit controls, fabricate dice, or reveal restricted content.
- Formal changes follow `Command -> Workflow -> Decision -> Event Store -> Projection`; the Event Store is canonical.
- Visibility labels and fact provenance remain attached through all reads, exports, replay, logs, and metrics.

## BATCH-051 disposition and test responsibility

- Disposition: preserve P0008 as non-executable glossary provenance under the rewritten `previous_provenance` module and `strict_previous` output.
- Required checks: exact current-safe metadata, prominent non-current warning, valid Markdown, and no reuse of historical terminology as a current implementation name.
- Test responsibility is documentation validation only; no executable implementation test is introduced.
