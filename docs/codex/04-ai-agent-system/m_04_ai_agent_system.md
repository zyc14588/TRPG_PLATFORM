# m_04_ai_agent_system

Status: current-safe governance index for `BATCH-017-04-ai-agent-system`.

## Scope

This document records the active AI agent system construction boundary after applying:

- `docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`
- `docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md`
- `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`

Historical source paths and legacy version names remain provenance only. They are not current module names, migration names, event names, metric names, workflow names, test names, or output names.

## Current-Safe Runtime Boundary

The current implementation boundary is `trpg-agent-runtime` with the `agent_runtime` module family.

Agents may create proposals, tool requests, and draft decisions. They may not directly write formal state, bypass the rules service, bypass state service, bypass event logging, forge dice, mutate an authority contract, or reveal visibility-restricted facts.

All provider access is represented as governance data and must follow:

`Agent Gateway -> Agent Orchestrator/Runtime -> Model Provider Adapter`

No business layer or agent runtime code in this batch directly calls OpenAI, Ollama, llama.cpp, or any other bare LLM endpoint.

## Batch Evidence

Batch-specific plan, traceability, test evidence, acceptance report, and handoff notes are under:

`evidence/batches/BATCH-017/`
