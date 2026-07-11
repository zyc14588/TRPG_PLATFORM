# ChatGPT follow-up research provenance

> BATCH-051 current-safe documentation output. This is a thin provenance page;
> the reusable current prompt structure is `followup_research_prompts.md`.

## Current-safe ownership

| Prompt ID | Prompt file | Current crate | Current module | Current output | Source file (provenance only) | Source SHA256 |
|---|---|---|---|---|---|---|
| `CODEX-1076-99-APPENDIX-1cf7a45b32` | `codex-prompts/99-appendix/P0004.md` | `trpg-docs-governance` | `appendix_research::chat_gpt` | `docs/codex/99-appendix/chat_gpt.md` | `docs/implementation/90-traceability/per-file-code-ready/99-appendix/docs-implementation-99-appendix-generated-from-source-strict-docs-implementation-99-appendix-followup-research-45260042b9.v5-code-ready.md` | `b7fca2759c519c6a93229e22018046c3f0f4e842198ff6fdbd0853c2a50b475c` |
| `CODEX-1081-99-APPENDIX-a93ccea365` | `codex-prompts/99-appendix/P0010.md` | `trpg-docs-governance` | `appendix_research::chat_gpt` | `docs/codex/99-appendix/chat_gpt.md` | `docs/implementation/90-traceability/per-file-code-ready/99-appendix/docs-implementation-99-appendix-generated-from-source-strict-docs-prompts-chatgpt-followup-research-prompts-st-6a3cbfa0ac.v5-code-ready.md` | `796d147a9e66dd6bef3a53925a5953fcf417d8de54eb99f6845c022c4f5809a7` |

## Disposition

- The provider name in the source lineage does not create a direct model call
  path or a preferred provider.
- Any future AI-assisted research still routes through the project's governed
  Agent path when it becomes product functionality.
- Historical implementation sketches remain provenance; this page owns no
  implementation or executable prompt.

The governance boundary remains unchanged: the Authority Contract is
immutable; `HUMAN_KP` and `AI_KP` are mutually exclusive; AI access uses the
Agent Gateway; formal state reaches the canonical Event Store only through the
governed decision path; Visibility and Fact Provenance remain end to end; and
the Policy Gate remains default-deny.

## Test responsibility

B051 verifies both Prompt IDs, the exact shared current-safe target, source
metadata, and absence of direct provider calls, product code, and formal state
writes.
