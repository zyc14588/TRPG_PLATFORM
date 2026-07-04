# Supplemental Requirement Merge

- Prompt ID: `CODEX-0418-03-RUNTIME-ORCHESTRATION-d4892063b3`
- Prompt file: `codex-prompts/03-runtime-orchestration/P0098.md`
- Primary Prompt: `CODEX-0032-03-RUNTIME-ORCHESTRATION-20830a72ac`
- Current module: `runtime_orchestration::capability_tool_grant`
- Status: pending merge by primary prompt; no BATCH-015 Rust output claimed.

Merge instructions for the primary prompt:

- Bind every capability grant to actor, campaign, session, authority mode, visibility scope, expiration, and tool permission.
- Require Agent Gateway and Tool Permission Gate approval before agent-visible tool execution.
- Keep tool results provenance-labeled and visibility-filtered before they enter agent context, realtime deltas, summary, RAG, export, or replay.
- Reject direct LLM/provider calls and direct database writes from capability evaluation code.

Suggested test assertions for the primary prompt:

- Expired, wrong-actor, wrong-authority, and wrong-visibility grants are denied.
- HUMAN_KP mode downgrades AI output to draft-only when a grant would otherwise permit formal action.
- Tool result provenance and visibility labels are preserved through replay-facing records.

This supplemental prompt remains traceability-only and does not declare a Rust source, test, migration, API handler, event schema, NATS subject, metric, workflow, or formal write path.
