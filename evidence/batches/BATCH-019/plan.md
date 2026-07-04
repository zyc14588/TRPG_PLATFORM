# BATCH-019 Work Plan

Batch: `BATCH-019-04-ai-agent-system -- Strict Governance Final`
Stage: `S07`
Category: `04-ai-agent-system`

## Read Inputs

- `AGENTS.md`
- `CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`
- `SOURCE_BUNDLE_INTEGRATION_GUIDE.md`
- `docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md`
- `docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`
- `docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md`
- `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`
- `batches/B019.md`
- `stages/s07-agent-runtime-provider-memory-rag/*`
- `docs/codex/04-ai-agent-system/*`
- `codex-prompts/04-ai-agent-system/P0051.md` through `P0075.md`

## Scope Decision

The user supplied "identified primary prompt count: 0", but the current-safe normalized maps and `batches/B019.md` identify four primary implementation prompts: `CODEX-0483`, `CODEX-0484`, `CODEX-0485`, and `CODEX-0507`. Current repository authority was followed.

## Prompt Plan

| Prompt ID | Role | Current-safe target | Allowed change | Test responsibility |
|---|---|---|---|---|
| `CODEX-0483-04-AI-AGENT-SYSTEM-a577767984` | primary | `crates/trpg-agent-runtime/src/memory_rag_impl.rs` | Rust module plus matching contract test | `cargo test -p trpg-agent-runtime --test memory_rag_impl_contract_tests` |
| `CODEX-0484-04-AI-AGENT-SYSTEM-e96dc3868d` | primary | `crates/trpg-agent-runtime/src/model_provider_local_cloud_impl.rs` | Rust module plus matching contract test | `cargo test -p trpg-agent-runtime --test model_provider_local_cloud_impl_contract_tests` |
| `CODEX-0485-04-AI-AGENT-SYSTEM-962b774429` | primary | `crates/trpg-agent-runtime/src/rag_snapshot_impl.rs` | Rust module plus matching contract test | `cargo test -p trpg-agent-runtime --test rag_snapshot_impl_contract_tests` |
| `CODEX-0486-04-AI-AGENT-SYSTEM-9ce89f19f8` | docs | `docs/codex/04-ai-agent-system/source_processing_record_docs_adr_adr_0009_agent_governance.md` | Markdown traceability only | Markdown existence and no Rust-output claim |
| `CODEX-0487-04-AI-AGENT-SYSTEM-dbe6de7e59` | docs | `docs/codex/04-ai-agent-system/source_processing_record_docs_adr_adr_0010_rag_snapshot.md` | Markdown traceability only | Markdown existence and no Rust-output claim |
| `CODEX-0488-04-AI-AGENT-SYSTEM-03fc2209c6` | docs | `docs/codex/04-ai-agent-system/source_processing_record_docs_ai_agent_runtime.md` | Markdown traceability only | Markdown existence and no Rust-output claim |
| `CODEX-0489-04-AI-AGENT-SYSTEM-752b9c9430` | docs | `docs/codex/04-ai-agent-system/source_processing_record_docs_ai_evaluation_golden_scenario.md` | Markdown traceability only | Markdown existence and no Rust-output claim |
| `CODEX-0490-04-AI-AGENT-SYSTEM-475b10a2a4` | docs | `docs/codex/04-ai-agent-system/source_processing_record_docs_ai_memory_rag.md` | Markdown traceability only | Markdown existence and no Rust-output claim |
| `CODEX-0491-04-AI-AGENT-SYSTEM-a7c5faa922` | docs | `docs/codex/04-ai-agent-system/source_processing_record_docs_ai_model_provider_local_cloud.md` | Markdown traceability only | Markdown existence and no Rust-output claim |
| `CODEX-0492-04-AI-AGENT-SYSTEM-f219f76442` | docs | `docs/codex/04-ai-agent-system/source_processing_record_docs_ai_rag_snapshot.md` | Markdown traceability only | Markdown existence and no Rust-output claim |
| `CODEX-0493-04-AI-AGENT-SYSTEM-eb040218e6` | docs | `docs/codex/04-ai-agent-system/source_processing_record_docs_implementation_04_ai_agent_system_readme.md` | Markdown traceability only | Markdown existence and no Rust-output claim |
| `CODEX-0494-04-AI-AGENT-SYSTEM-e007c89f57` | docs | `docs/codex/04-ai-agent-system/source_processing_record_docs_implementation_04_ai_agent_system_local_model_certification.md` | Markdown traceability only | Markdown existence and no Rust-output claim |
| `CODEX-0495-04-AI-AGENT-SYSTEM-799fc14dc2` | docs | `docs/codex/04-ai-agent-system/source_processing_record_docs_implementation_04_ai_agent_system_model_provider.md` | Markdown traceability only | Markdown existence and no Rust-output claim |
| `CODEX-0496-04-AI-AGENT-SYSTEM-c0f67c85c7` | docs | `docs/codex/04-ai-agent-system/source_processing_record_docs_implementation_04_ai_agent_system_memory_rag_rag_snapshot.md` | Markdown traceability only | Markdown existence and no Rust-output claim |
| `CODEX-0497-04-AI-AGENT-SYSTEM-044ab5dc87` | docs | `docs/codex/04-ai-agent-system/source_processing_record_docs_implementation_04_ai_agent_system_ai_evaluation_runtime.md` | Markdown traceability only | Markdown existence and no Rust-output claim |
| `CODEX-0498-04-AI-AGENT-SYSTEM-13927ff7ed` | docs | `docs/codex/04-ai-agent-system/source_processing_record_docs_implementation_04_ai_agent_system_agent_context_assembler.md` | Markdown traceability only | Markdown existence and no Rust-output claim |
| `CODEX-0499-04-AI-AGENT-SYSTEM-04b8aaf7da` | docs | `docs/codex/04-ai-agent-system/source_processing_record_docs_implementation_04_ai_agent_system_tool_protocol.md` | Markdown traceability only | Markdown existence and no Rust-output claim |
| `CODEX-0500-04-AI-AGENT-SYSTEM-9f239edf80` | docs | `docs/codex/04-ai-agent-system/source_processing_record_docs_implementation_04_ai_agent_system_agent_runtime.md` | Markdown traceability only | Markdown existence and no Rust-output claim |
| `CODEX-0501-04-AI-AGENT-SYSTEM-687782b527` | docs | `docs/codex/04-ai-agent-system/source_processing_record_docs_implementation_90_traceability_source_breakdown_ai_evaluation_golden_scenario.md` | Markdown traceability only | Markdown existence and no Rust-output claim |
| `CODEX-0502-04-AI-AGENT-SYSTEM-1cc19ac6d6` | docs | `docs/codex/04-ai-agent-system/source_processing_record_docs_implementation_90_traceability_source_breakdown_ai_agent_runtime.md` | Markdown traceability only | Markdown existence and no Rust-output claim |
| `CODEX-0503-04-AI-AGENT-SYSTEM-0e7645f3a5` | docs | `docs/codex/04-ai-agent-system/source_processing_record_docs_implementation_90_traceability_source_breakdown_ai_model_provider_local_cloud.md` | Markdown traceability only | Markdown existence and no Rust-output claim |
| `CODEX-0504-04-AI-AGENT-SYSTEM-2d75990472` | docs | `docs/codex/04-ai-agent-system/source_processing_record_docs_implementation_90_traceability_source_breakdown_ai_rag_snapshot.md` | Markdown traceability only | Markdown existence and no Rust-output claim |
| `CODEX-0505-04-AI-AGENT-SYSTEM-9f37999d40` | docs | `docs/codex/04-ai-agent-system/source_processing_record_docs_implementation_90_traceability_source_breakdown_ai_memory_rag.md` | Markdown traceability only | Markdown existence and no Rust-output claim |
| `CODEX-0506-04-AI-AGENT-SYSTEM-e75d4617db` | docs | `docs/codex/04-ai-agent-system/source_processing_record_docs_implementation_12_extension_sdk_agent_pack_sdk.md` | Markdown traceability only | Markdown existence and no Rust-output claim |
| `CODEX-0507-04-AI-AGENT-SYSTEM-a1e5d3d499` | primary | `crates/trpg-agent-runtime/src/adr_0009_agent_governance.rs` | Rust module plus matching contract test | `cargo test -p trpg-agent-runtime --test adr_0009_agent_governance_contract_tests` |

## Explicit Non-Scope

- No API handler changes.
- No migration changes.
- No NATS subject changes.
- No direct model/provider HTTP calls.
- No later batch prompts.
