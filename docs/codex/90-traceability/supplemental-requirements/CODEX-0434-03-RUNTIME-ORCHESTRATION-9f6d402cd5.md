# Supplemental Requirement Merge

- Prompt ID: `CODEX-0434-03-RUNTIME-ORCHESTRATION-9f6d402cd5`
- Prompt file: `codex-prompts/03-runtime-orchestration/P0109.md`
- Primary Prompt: `CODEX-0346-03-RUNTIME-ORCHESTRATION-fc8679858e`
- Current module: `runtime_orchestration::capability_layer`
- Status: pending merge by primary prompt; no BATCH-016 Rust output claimed.

Merge instructions for the primary prompt:

- Capability layer must mediate formal tools and runtime capabilities before any state-changing decision.
- Tool grants must respect Authority Contract, HUMAN_KP / AI_KP exclusivity, Policy Gate, Visibility Label, and Fact Provenance.
- Capability decisions must be observable without leaking restricted prompt, tool, or game facts.

Suggested test assertions for the primary prompt:

- Default-deny blocks unknown capabilities and appends no canonical event.
- HUMAN_KP mode downgrades AI formal tool attempts to draft-only.
- AI_KP mode rejects non-orchestrator formal adjudication requests.

This supplemental prompt remains traceability-only and does not declare a Rust source, test, migration, API handler, event schema, NATS subject, metric, workflow, or formal write path.
