# Previous Follow-up Prompt Provenance

> This page is provenance only. It is not a current execution, implementation, or acceptance entry point.

## Current-safe metadata

| Prompt ID | Prompt path | Source path | Source SHA256 | Crate | Current-safe module | Current-safe output |
|---|---|---|---|---|---|---|
| `CODEX-1073-99-APPENDIX-35667c12b9` | `codex-prompts/99-appendix/P0002.md` | `docs/implementation/90-traceability/per-file-code-ready/99-appendix/docs-implementation-99-appendix-followup-prompts-v4-75d3b185ba.v5-code-ready.md` | `cb001c4f9af788feef8006b2ecbf6cd839c47ef4cb6a784ddba2230655488e53` | `trpg-docs-governance` | `appendix_research::followup_prompts_previous_provenance` | `docs/codex/99-appendix/followup_prompts_previous.md` |

The source path and hash identify the retained input only; they are not current names or requirements.

## Allowed boundary

- Output role: `documentation-or-traceability`.
- This page may record provenance, disposition, and validation responsibility for P0002.
- It must not revive historical follow-up instructions, define current scope, or become an acceptance authority.
- Any actionable work must originate from the current top-level design, current-safe maps, and an authorized current batch.

## Governance red lines

- Authority Contracts remain immutable and authority changes are fork-only.
- `HUMAN_KP` and `AI_KP` remain mutually exclusive at campaign level.
- AI access follows the Agent Gateway and governed runtime; business code does not call model providers directly.
- AI does not write canonical state, bypass rules or policy gates, fabricate dice, or disclose restricted visibility.
- Formal state follows `Command -> Workflow -> Decision -> Event Store -> Projection`; the Event Store remains canonical.
- Visibility and fact provenance remain attached across every downstream representation and audit surface.

## BATCH-051 disposition and test responsibility

- Disposition: retain P0002 as previous-prompt provenance in the current-safe output above; do not execute its historical proposals.
- Required checks: exact Prompt ID/path/SHA mapping, explicit non-current warning, valid Markdown, and absence of concrete code or contract outputs.
- Test responsibility is documentation validation only; no executable implementation test is introduced.
