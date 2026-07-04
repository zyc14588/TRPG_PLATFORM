# BATCH-017 Prompt Traceability

Batch file: `batches/B017.md`

Declared prompt count: 25

Current-safe execution result: all 25 prompt IDs were read, mapped, and accounted for.

## Primary Implementation Prompts

The current-safe maps identify these 16 primary implementation prompts. They were implemented in `trpg-agent-runtime` flat modules and covered by `crates/trpg-agent-runtime/tests/batch_017_agent_runtime_contract_tests.rs`.

- `CODEX-0041-04-AI-AGENT-SYSTEM-570f17da9d`
- `CODEX-0042-04-AI-AGENT-SYSTEM-bbc851a5de`
- `CODEX-0043-04-AI-AGENT-SYSTEM-8bea44b53b`
- `CODEX-0044-04-AI-AGENT-SYSTEM-4a4aa2a8df`
- `CODEX-0045-04-AI-AGENT-SYSTEM-e852321d0b`
- `CODEX-0046-04-AI-AGENT-SYSTEM-6468c9be5b`
- `CODEX-0047-04-AI-AGENT-SYSTEM-524e2550a8`
- `CODEX-0441-04-AI-AGENT-SYSTEM-b81eba8b66`
- `CODEX-0444-04-AI-AGENT-SYSTEM-20434579ca`
- `CODEX-0447-04-AI-AGENT-SYSTEM-3497400719`
- `CODEX-0448-04-AI-AGENT-SYSTEM-41ecd49e88`
- `CODEX-0450-04-AI-AGENT-SYSTEM-3e566913fa`
- `CODEX-0452-04-AI-AGENT-SYSTEM-4f2dab7f75`
- `CODEX-0454-04-AI-AGENT-SYSTEM-014bc53177`
- `CODEX-0456-04-AI-AGENT-SYSTEM-d68068a022`
- `CODEX-0457-04-AI-AGENT-SYSTEM-487c497469`

## Documentation / Traceability Prompt

- `CODEX-0040-04-AI-AGENT-SYSTEM-0ed30fc5f0`
  - Current-safe output: `docs/codex/04-ai-agent-system/m_04_ai_agent_system.md`

## Supplemental Requirement Prompts

These prompts were treated as supplemental only. They did not create independent Rust outputs. Their constraints were merged into the corresponding primary module tests and implementation behavior.

- `CODEX-0442-04-AI-AGENT-SYSTEM-34a7e5c6f0` -> merged into `agent_context_assembler`
- `CODEX-0443-04-AI-AGENT-SYSTEM-bcbd7b78de` -> merged into `agent_runtime`
- `CODEX-0445-04-AI-AGENT-SYSTEM-43507a6209` -> merged into `ai_evaluation_runtime`
- `CODEX-0446-04-AI-AGENT-SYSTEM-bafcf3dfc6` -> merged into `agent_runtime`
- `CODEX-0449-04-AI-AGENT-SYSTEM-b319601824` -> merged into `model_provider`
- `CODEX-0451-04-AI-AGENT-SYSTEM-dab850ee74` -> merged into `agent_runtime`
- `CODEX-0453-04-AI-AGENT-SYSTEM-159b37a04c` -> merged into `model_provider`
- `CODEX-0455-04-AI-AGENT-SYSTEM-a49d9b14ee` -> merged into `agent_runtime`

## Batch Fact Discrepancy

`batch-prompts/start/B017.md` states that the recognized primary prompt count is 0. The batch file, per-file manifest, and current-safe maps identify 16 primary implementation rows. This run followed the current-safe maps and recorded the discrepancy in `plan.md` and `handoff.md`.
