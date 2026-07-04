# BATCH-020 Work Plan

Batch: BATCH-020-04-ai-agent-system - Strict Governance Final
Stage: S07 agent runtime / provider / memory / RAG
Declared prompts: 20

Executor note: the runtime instruction supplied "recognized primary prompt count: 0". The current repository authorities used for execution are `batches/B020.md` plus `docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md`; those mark `CODEX-0508` and `CODEX-0510` as primary. This evidence records the discrepancy and follows the repository authority order.

## Prompt Mapping

| Prompt file | Prompt ID | Current-safe output target | Allowed scope | Test responsibility |
| --- | --- | --- | --- | --- |
| `codex-prompts/04-ai-agent-system/P0076.md` | `CODEX-0508-04-AI-AGENT-SYSTEM-f2ee9f2b79` | `crates/trpg-agent-runtime/src/adr_0010_rag_snapshot.rs` | Primary implementation only in `trpg-agent-runtime`, plus crate exports and focused tests. No DB/API/NATS/LLM/provider call. | `cargo test -p trpg-agent-runtime --test adr_0010_rag_snapshot_contract_tests`; stage crate tests. |
| `codex-prompts/04-ai-agent-system/P0077.md` | `CODEX-0509-04-AI-AGENT-SYSTEM-90fc5447c3` | `codex-prompts/04-ai-agent-system/P0077.md` | Supplemental only: constraints, test expectations, traceability. No implementation file changes from this prompt. | Covered by prompt coverage evidence and stage governance checks. |
| `codex-prompts/04-ai-agent-system/P0078.md` | `CODEX-0510-04-AI-AGENT-SYSTEM-c10997b277` | `crates/trpg-agent-runtime/src/evaluation_golden_scenario.rs` | Primary implementation only in `trpg-agent-runtime`, plus crate exports and focused tests. No DB/API/NATS/LLM/provider call. | `cargo test -p trpg-agent-runtime --test evaluation_golden_scenario_contract_tests`; stage crate tests. |
| `codex-prompts/04-ai-agent-system/P0079.md` | `CODEX-0511-04-AI-AGENT-SYSTEM-d4b544c710` | `codex-prompts/04-ai-agent-system/P0079.md` | Supplemental only: constraints, test expectations, traceability. No implementation file changes from this prompt. | Covered by prompt coverage evidence and stage governance checks. |
| `codex-prompts/04-ai-agent-system/P0080.md` | `CODEX-0512-04-AI-AGENT-SYSTEM-9aca88599f` | `codex-prompts/04-ai-agent-system/P0080.md` | Supplemental only: constraints, test expectations, traceability. No implementation file changes from this prompt. | Covered by prompt coverage evidence and stage governance checks. |
| `codex-prompts/04-ai-agent-system/P0081.md` | `CODEX-0513-04-AI-AGENT-SYSTEM-61890cfc3d` | `codex-prompts/04-ai-agent-system/P0081.md` | Supplemental only: constraints, test expectations, traceability. No implementation file changes from this prompt. | Covered by prompt coverage evidence and stage governance checks. |
| `codex-prompts/04-ai-agent-system/P0084.md` | `CODEX-0514-04-AI-AGENT-SYSTEM-a5ddc4c4c8` | `codex-prompts/04-ai-agent-system/P0084.md` | Supplemental only: constraints, test expectations, traceability. No implementation file changes from this prompt. | Covered by prompt coverage evidence and stage governance checks. |
| `codex-prompts/04-ai-agent-system/P0082.md` | `CODEX-0515-04-AI-AGENT-SYSTEM-3d03dccf07` | `codex-prompts/04-ai-agent-system/P0082.md` | Supplemental only: constraints, test expectations, traceability. No implementation file changes from this prompt. | Covered by prompt coverage evidence and stage governance checks. |
| `codex-prompts/04-ai-agent-system/P0083.md` | `CODEX-0516-04-AI-AGENT-SYSTEM-9146c6434e` | `codex-prompts/04-ai-agent-system/P0083.md` | Supplemental only: constraints, test expectations, traceability. No implementation file changes from this prompt. | Covered by prompt coverage evidence and stage governance checks. |
| `codex-prompts/04-ai-agent-system/P0085.md` | `CODEX-0517-04-AI-AGENT-SYSTEM-43ed30f2e9` | `codex-prompts/04-ai-agent-system/P0085.md` | Supplemental only: constraints, test expectations, traceability. No implementation file changes from this prompt. | Covered by prompt coverage evidence and stage governance checks. |
| `codex-prompts/04-ai-agent-system/P0086.md` | `CODEX-0518-04-AI-AGENT-SYSTEM-b0096db6a4` | `codex-prompts/04-ai-agent-system/P0086.md` | Supplemental only: constraints, test expectations, traceability. No implementation file changes from this prompt. | Covered by prompt coverage evidence and stage governance checks. |
| `codex-prompts/04-ai-agent-system/P0087.md` | `CODEX-0519-04-AI-AGENT-SYSTEM-bd4d1ae282` | `codex-prompts/04-ai-agent-system/P0087.md` | Supplemental only: constraints, test expectations, traceability. No implementation file changes from this prompt. | Covered by prompt coverage evidence and stage governance checks. |
| `codex-prompts/04-ai-agent-system/P0088.md` | `CODEX-0520-04-AI-AGENT-SYSTEM-e81ac9192d` | `codex-prompts/04-ai-agent-system/P0088.md` | Supplemental only: constraints, test expectations, traceability. No implementation file changes from this prompt. | Covered by prompt coverage evidence and stage governance checks. |
| `codex-prompts/04-ai-agent-system/P0089.md` | `CODEX-0521-04-AI-AGENT-SYSTEM-0a9a11d351` | `codex-prompts/04-ai-agent-system/P0089.md` | Supplemental only: constraints, test expectations, traceability. No implementation file changes from this prompt. | Covered by prompt coverage evidence and stage governance checks. |
| `codex-prompts/04-ai-agent-system/P0090.md` | `CODEX-0522-04-AI-AGENT-SYSTEM-0979831cd7` | `codex-prompts/04-ai-agent-system/P0090.md` | Supplemental only: constraints, test expectations, traceability. No implementation file changes from this prompt. | Covered by prompt coverage evidence and stage governance checks. |
| `codex-prompts/04-ai-agent-system/P0095.md` | `CODEX-0523-04-AI-AGENT-SYSTEM-e5a5c03c2c` | `codex-prompts/04-ai-agent-system/P0095.md` | Supplemental only: constraints, test expectations, traceability. No implementation file changes from this prompt. | Covered by prompt coverage evidence and stage governance checks. |
| `codex-prompts/04-ai-agent-system/P0092.md` | `CODEX-0524-04-AI-AGENT-SYSTEM-43adbfc936` | `codex-prompts/04-ai-agent-system/P0092.md` | Supplemental only: constraints, test expectations, traceability. No implementation file changes from this prompt. | Covered by prompt coverage evidence and stage governance checks. |
| `codex-prompts/04-ai-agent-system/P0094.md` | `CODEX-0525-04-AI-AGENT-SYSTEM-adbdea50ff` | `codex-prompts/04-ai-agent-system/P0094.md` | Supplemental only: constraints, test expectations, traceability. No implementation file changes from this prompt. | Covered by prompt coverage evidence and stage governance checks. |
| `codex-prompts/04-ai-agent-system/P0093.md` | `CODEX-0526-04-AI-AGENT-SYSTEM-934a081c8e` | `codex-prompts/04-ai-agent-system/P0093.md` | Supplemental only: constraints, test expectations, traceability. No implementation file changes from this prompt. | Covered by prompt coverage evidence and stage governance checks. |
| `codex-prompts/04-ai-agent-system/P0091.md` | `CODEX-0527-04-AI-AGENT-SYSTEM-3d3a1f2aad` | `codex-prompts/04-ai-agent-system/P0091.md` | Supplemental only: constraints, test expectations, traceability. No implementation file changes from this prompt. | Covered by prompt coverage evidence and stage governance checks. |

## Execution Plan

1. Read governing files, stage guides, B020, current-safe maps, and all referenced per-file prompts.
2. Apply normalized current-safe mappings before implementation.
3. Implement only the two mapped primary Rust modules in `trpg-agent-runtime`.
4. Add focused contract tests for RAG snapshot governance and golden scenario governance.
5. Run minimal tests, then stage and workspace checks.
6. Write B020 evidence under `evidence/batches/BATCH-020/`.
