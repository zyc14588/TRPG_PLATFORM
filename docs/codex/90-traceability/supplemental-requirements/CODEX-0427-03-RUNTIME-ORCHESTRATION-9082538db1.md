# Supplemental Requirement Merge

- Prompt ID: `CODEX-0427-03-RUNTIME-ORCHESTRATION-9082538db1`
- Prompt file: `codex-prompts/03-runtime-orchestration/P0104.md`
- Primary Prompt: `CODEX-0386-03-RUNTIME-ORCHESTRATION-027bb089fe`
- Current module: `runtime_orchestration::capability_layer_impl`
- Status: pending merge by primary prompt; no BATCH-016 Rust output claimed.

Merge instructions for the primary prompt:

- Capability checks must be default-deny and must not bypass Authority Contract, Tool Grant, Policy Gate, Visibility Label, or Fact Provenance.
- HUMAN_KP mode must downgrade AI formal tool attempts to draft-only proposals.
- AI_KP mode must allow formal adjudication tools only through the authorized AI Keeper Orchestrator path.

Suggested test assertions for the primary prompt:

- Unauthorized capabilities append no canonical event and return a policy/authority denial.
- Capability grants retain actor, authority mode, correlation id, causation id, visibility, and provenance.
- Private tool results are excluded from unauthorized realtime, replay, and agent-context views.

This supplemental prompt remains traceability-only and does not declare a Rust source, test, migration, API handler, event schema, NATS subject, metric, workflow, or formal write path.
