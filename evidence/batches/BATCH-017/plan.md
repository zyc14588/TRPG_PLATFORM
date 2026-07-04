# BATCH-017 Work Plan

Batch: `BATCH-017-04-ai-agent-system -- Strict Governance Final`

Stage: `S07 -- Agent Runtime: Gateway, Tool Permission Gate, Provider, Local Model Certification, Memory/RAG`

Scope rule: only `batches/B017.md` rows are in scope. `source-archive/**` is provenance only. Current-safe mappings from `CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md` and `CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md` override historical source-path names.

Input inconsistency: `batch-prompts/start/B017.md` says "recognized primary prompt count: 0", but `batches/B017.md`, `per-file-prompt-manifest.md`, and current-safe maps identify 16 primary implementation rows. This plan follows the stronger current-safe batch mapping and records the discrepancy as a risk.

## Prompt Mapping

| Prompt ID | Role | Current-safe target | Allowed change | Test responsibility |
|---|---|---|---|---|
| `CODEX-0040-04-AI-AGENT-SYSTEM-0ed30fc5f0` | documentation-or-traceability | `docs/codex/04-ai-agent-system/m_04_ai_agent_system.md` | Documentation/traceability only. | Covered by prompt traceability evidence. |
| `CODEX-0041-04-AI-AGENT-SYSTEM-570f17da9d` | primary-implementation | `crates/trpg-agent-runtime/src/agent_context_assembler.rs` | Create/update flat Rust module in `trpg-agent-runtime`. | Context visibility/provenance contract tests. |
| `CODEX-0042-04-AI-AGENT-SYSTEM-bbc851a5de` | primary-implementation | `crates/trpg-agent-runtime/src/agent_runtime.rs` | Create/update Agent Gateway/Runtime boundary. | Tool gate, evented decision, direct-write denial tests. |
| `CODEX-0043-04-AI-AGENT-SYSTEM-8bea44b53b` | primary-implementation | `crates/trpg-agent-runtime/src/ai_evaluation_runtime.rs` | Create/update evaluation runtime guard. | Evaluation redaction and audit-field tests. |
| `CODEX-0044-04-AI-AGENT-SYSTEM-4a4aa2a8df` | primary-implementation | `crates/trpg-agent-runtime/src/local_model_certification.rs` | Create/update local model certification logic. | Level 4 AI Keeper gate tests. |
| `CODEX-0045-04-AI-AGENT-SYSTEM-e852321d0b` | primary-implementation | `crates/trpg-agent-runtime/src/memory_rag_rag_snapshot.rs` | Create/update RAG snapshot wrapper. | RAG metadata/visibility tests. |
| `CODEX-0046-04-AI-AGENT-SYSTEM-6468c9be5b` | primary-implementation | `crates/trpg-agent-runtime/src/model_provider.rs` | Create/update provider boundary logic. | Provider security and direct-call boundary tests. |
| `CODEX-0047-04-AI-AGENT-SYSTEM-524e2550a8` | primary-implementation | `crates/trpg-agent-runtime/src/tool_protocol.rs` | Create/update tool protocol gate. | HUMAN_KP draft-only and AI_KP tool tests. |
| `CODEX-0441-04-AI-AGENT-SYSTEM-b81eba8b66` | primary-implementation | `crates/trpg-agent-runtime/src/adr_0009_agent_governance_agent_governance.rs` | Create/update governance ADR executable contract. | Governance snapshot test. |
| `CODEX-0442-04-AI-AGENT-SYSTEM-34a7e5c6f0` | supplemental-requirement | `codex-prompts/04-ai-agent-system/P0010.md` | No Rust output; merge into `agent_context_assembler`. | Trace as supplemental-applied. |
| `CODEX-0443-04-AI-AGENT-SYSTEM-bcbd7b78de` | supplemental-requirement | `codex-prompts/04-ai-agent-system/P0011.md` | No Rust output; merge into `agent_runtime`. | Trace as supplemental-applied. |
| `CODEX-0444-04-AI-AGENT-SYSTEM-20434579ca` | primary-implementation | `crates/trpg-agent-runtime/src/agent_runtime_tool_protocol.rs` | Create/update runtime/tool protocol bridge. | Tool gate bridge tests. |
| `CODEX-0445-04-AI-AGENT-SYSTEM-43507a6209` | supplemental-requirement | `codex-prompts/04-ai-agent-system/P0013.md` | No Rust output; merge into `ai_evaluation_runtime`. | Trace as supplemental-applied. |
| `CODEX-0446-04-AI-AGENT-SYSTEM-bafcf3dfc6` | supplemental-requirement | `codex-prompts/04-ai-agent-system/P0014.md` | No Rust output; merge into `agent_runtime`. | Trace as supplemental-applied. |
| `CODEX-0447-04-AI-AGENT-SYSTEM-3497400719` | primary-implementation | `crates/trpg-agent-runtime/src/agent_evaluation_golden_scenario.rs` | Create/update golden scenario evaluator. | Prompt-injection and export redaction tests. |
| `CODEX-0448-04-AI-AGENT-SYSTEM-41ecd49e88` | primary-implementation | `crates/trpg-agent-runtime/src/working_memory_long_memory_rag.rs` | Create/update memory/RAG visibility wrapper. | Keeper-only exclusion tests. |
| `CODEX-0449-04-AI-AGENT-SYSTEM-b319601824` | supplemental-requirement | `codex-prompts/04-ai-agent-system/P0017.md` | No Rust output; merge into `model_provider`. | Trace as supplemental-applied. |
| `CODEX-0450-04-AI-AGENT-SYSTEM-3e566913fa` | primary-implementation | `crates/trpg-agent-runtime/src/rag_snapshot.rs` | Create/update RAG chunk/snapshot logic. | Public allow and keeper-only deny tests. |
| `CODEX-0451-04-AI-AGENT-SYSTEM-dab850ee74` | supplemental-requirement | `codex-prompts/04-ai-agent-system/P0026.md` | No Rust output; merge into `agent_runtime`. | Trace as supplemental-applied. |
| `CODEX-0452-04-AI-AGENT-SYSTEM-4f2dab7f75` | primary-implementation | `crates/trpg-agent-runtime/src/working_memory_rag_rag_snapshot.rs` | Create/update working-memory RAG snapshot wrapper. | RAG snapshot contract tests. |
| `CODEX-0453-04-AI-AGENT-SYSTEM-159b37a04c` | supplemental-requirement | `codex-prompts/04-ai-agent-system/P0020.md` | No Rust output; merge into `model_provider`. | Trace as supplemental-applied. |
| `CODEX-0454-04-AI-AGENT-SYSTEM-014bc53177` | primary-implementation | `crates/trpg-agent-runtime/src/model_provider_local_cloud.rs` | Create/update no-silent-fallback boundary. | Local/cloud fallback tests. |
| `CODEX-0455-04-AI-AGENT-SYSTEM-a49d9b14ee` | supplemental-requirement | `codex-prompts/04-ai-agent-system/P0023.md` | No Rust output; merge into `agent_runtime`. | Trace as supplemental-applied. |
| `CODEX-0456-04-AI-AGENT-SYSTEM-d68068a022` | primary-implementation | `crates/trpg-agent-runtime/src/memory_rag.rs` | Create/update memory/RAG query boundary. | Visibility redaction tests. |
| `CODEX-0457-04-AI-AGENT-SYSTEM-487c497469` | primary-implementation | `crates/trpg-agent-runtime/src/ai_evaluation_golden_scenario.rs` | Create/update AI evaluation golden scenario wrapper. | Golden scenario prompt-injection tests. |

## Minimal Implementation Slice

1. Add `trpg-agent-runtime` as a workspace crate because all B017 product-code outputs target it and it does not exist yet.
2. Reuse `trpg-shared-kernel` for authority, command envelope, event store, visibility, and provenance. Do not create duplicate governance primitives.
3. Implement pure in-memory contract logic only: tool permission decisions, provider configuration validation, certification level checks, RAG visibility filtering, and evented agent decision append through `EventStore`.
4. Do not create migrations, API handlers, NATS subjects, or provider network clients in this batch. Their prompt references are recorded as deferred because B017 can satisfy strict governance with pure contracts and tests.
5. Add focused tests under `crates/trpg-agent-runtime/tests/` for B017 and S07 required fixture cases.

## Planned Commands

1. `cargo test -p trpg-agent-runtime --all-features`
2. `cargo test -p trpg-agent-runtime --test batch_017_agent_runtime_contract_tests --all-features`
3. `cargo test --workspace --all-features`

## Evidence Outputs

- `evidence/batches/BATCH-017/plan.md`
- `evidence/batches/BATCH-017/changed-files.txt`
- `evidence/batches/BATCH-017/test-output.txt`
- `evidence/batches/BATCH-017/prompt-traceability.md`
- `evidence/batches/BATCH-017/handoff.md`
- `evidence/batches/BATCH-017/acceptance-report.md`
