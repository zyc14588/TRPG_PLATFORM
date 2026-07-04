# S07 Acceptance Evidence - BATCH-017

Stage: S07 - Agent Runtime, Provider Adapter, Model Certification, RAG Visibility
Batch: BATCH-017-04-ai-agent-system
Evidence date: 2026-07-05
Repair scope: evidence only. No implementation code changed for this report.

## Evidence Sources

- `evidence/stages/S07/provider-adapter-tests.txt`
- `evidence/stages/S07/model-certification-tests.txt`
- `evidence/stages/S07/rag-visibility-tests.txt`
- `evidence/batches/BATCH-017/prompt-traceability.md`
- `evidence/batches/BATCH-017/test-output.txt`
- `evidence/batches/BATCH-017/acceptance-report.md`
- `fixtures/stages/S07_stage_acceptance_fixture.v1.json.md`
- `fixtures/stages/detailed/S07_provider_rag_model_cert_expected.current.json.md`
- `fixtures/provider/model_certification_matrix.v1.json.md`
- `fixtures/provider/agent_tool_gate_cases.v1.json.md`
- `fixtures/provider/rag_snapshot_cases.v1.json.md`

## Acceptance Summary

S07 acceptance evidence is complete for BATCH-017:

- B017 prompt coverage: 25/25 rows accounted.
- Role split: 16 primary implementation prompts, 8 supplemental requirement prompts, 1 docs-governance prompt.
- Primary prompts have implementation evidence and targeted contract tests.
- Supplemental prompts are merged into current-safe primary modules and do not introduce independent implementation scope.
- S07 fixtures cover `expected_events`, `expected_records`, `expected_errors`, and `pass_criteria`.
- Cargo test/check/clippy evidence is recorded in `S07_TEST_RESULTS.md`.
- `pnpm` and `docker` checks are N/A because no package or Docker manifests are present in this repository.

## Current Command Results

| Command | Current result |
| --- | --- |
| `cargo test -p trpg-agent-runtime --test batch_017_agent_runtime_contract_tests --all-features` | PASS: 12/12 B017 contract tests passed. |
| `cargo test -p trpg-agent-runtime --all-features` | PASS. |
| `cargo test -p trpg-domain-core --all-features` | PASS. |
| `cargo check --workspace --all-targets --all-features` | PASS. |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS. |
| `cargo test --workspace --all-features` | PASS. |

## Prompt Coverage

### Primary Implementation Prompts

| Prompt ID | Current-safe output | Evidence | Targeted test coverage |
| --- | --- | --- | --- |
| CODEX-0041-04-AI-AGENT-SYSTEM-570f17da9d | `agent_context_assembler` | `crates/trpg-agent-runtime/src/agent_context_assembler.rs` | `primary_wrapper_modules_call_entrypoints_and_cover_prompt_ids` |
| CODEX-0042-04-AI-AGENT-SYSTEM-bbc851a5de | `agent_runtime` | `crates/trpg-agent-runtime/src/agent_runtime.rs` | commit, tool gate, visibility, and formal-state negative tests |
| CODEX-0043-04-AI-AGENT-SYSTEM-8bea44b53b | `ai_evaluation_runtime` | `crates/trpg-agent-runtime/src/ai_evaluation_runtime.rs` | wrapper contract and prompt-injection fixture assertions |
| CODEX-0044-04-AI-AGENT-SYSTEM-4a4aa2a8df | `local_model_certification` | `crates/trpg-agent-runtime/src/local_model_certification.rs` | `local_model_certification_requires_level4_for_ai_keeper` |
| CODEX-0045-04-AI-AGENT-SYSTEM-e852321d0b | `memory_rag_rag_snapshot` | `crates/trpg-agent-runtime/src/memory_rag_rag_snapshot.rs` | wrapper and RAG visibility contract tests |
| CODEX-0046-04-AI-AGENT-SYSTEM-6468c9be5b | `model_provider` | `crates/trpg-agent-runtime/src/model_provider.rs` | provider boundary and fallback contract tests |
| CODEX-0047-04-AI-AGENT-SYSTEM-524e2550a8 | `tool_protocol` | `crates/trpg-agent-runtime/src/tool_protocol.rs` | agent tool gate fixture assertions |
| CODEX-0441-04-AI-AGENT-SYSTEM-b81eba8b66 | `adr_0009_agent_governance_agent_governance` | `crates/trpg-agent-runtime/src/adr_0009_agent_governance_agent_governance.rs` | wrapper governance contract test |
| CODEX-0444-04-AI-AGENT-SYSTEM-20434579ca | `agent_runtime_tool_protocol` | `crates/trpg-agent-runtime/src/agent_runtime_tool_protocol.rs` | wrapper and tool permission contract tests |
| CODEX-0447-04-AI-AGENT-SYSTEM-3497400719 | `agent_evaluation_golden_scenario` | `crates/trpg-agent-runtime/src/agent_evaluation_golden_scenario.rs` | wrapper and redaction contract tests |
| CODEX-0448-04-AI-AGENT-SYSTEM-41ecd49e88 | `working_memory_long_memory_rag` | `crates/trpg-agent-runtime/src/working_memory_long_memory_rag.rs` | wrapper and RAG visibility contract tests |
| CODEX-0450-04-AI-AGENT-SYSTEM-3e566913fa | `rag_snapshot` | `crates/trpg-agent-runtime/src/rag_snapshot.rs` | `s07_fixtures_drive_provider_model_rag_assertions` |
| CODEX-0452-04-AI-AGENT-SYSTEM-4f2dab7f75 | `working_memory_rag_rag_snapshot` | `crates/trpg-agent-runtime/src/working_memory_rag_rag_snapshot.rs` | wrapper and RAG visibility contract tests |
| CODEX-0454-04-AI-AGENT-SYSTEM-014bc53177 | `model_provider_local_cloud` | `crates/trpg-agent-runtime/src/model_provider_local_cloud.rs` | provider adapter boundary contract tests |
| CODEX-0456-04-AI-AGENT-SYSTEM-d68068a022 | `memory_rag` | `crates/trpg-agent-runtime/src/memory_rag.rs` | wrapper and RAG visibility contract tests |
| CODEX-0457-04-AI-AGENT-SYSTEM-487c497469 | `ai_evaluation_golden_scenario` | `crates/trpg-agent-runtime/src/ai_evaluation_golden_scenario.rs` | wrapper and redaction contract tests |

Primary coverage result: 16/16 PASS.

### Supplemental Requirement Prompts

| Prompt ID | Merge target | Acceptance conclusion |
| --- | --- | --- |
| CODEX-0442-04-AI-AGENT-SYSTEM-34a7e5c6f0 | `agent_context_assembler` | PASS: supplemental requirement merged into primary module evidence; no independent implementation scope. |
| CODEX-0443-04-AI-AGENT-SYSTEM-bcbd7b78de | `agent_runtime` | PASS: supplemental requirement merged into primary module evidence; no independent implementation scope. |
| CODEX-0445-04-AI-AGENT-SYSTEM-43507a6209 | `ai_evaluation_runtime` | PASS: supplemental requirement merged into primary module evidence; no independent implementation scope. |
| CODEX-0446-04-AI-AGENT-SYSTEM-bafcf3dfc6 | `agent_runtime` | PASS: supplemental requirement merged into primary module evidence; no independent implementation scope. |
| CODEX-0449-04-AI-AGENT-SYSTEM-b319601824 | `model_provider` | PASS: supplemental requirement merged into primary module evidence; no independent implementation scope. |
| CODEX-0451-04-AI-AGENT-SYSTEM-dab850ee74 | `agent_runtime` | PASS: supplemental requirement merged into primary module evidence; no independent implementation scope. |
| CODEX-0453-04-AI-AGENT-SYSTEM-159b37a04c | `model_provider` | PASS: supplemental requirement merged into primary module evidence; no independent implementation scope. |
| CODEX-0455-04-AI-AGENT-SYSTEM-a49d9b14ee | `agent_runtime` | PASS: supplemental requirement merged into primary module evidence; no independent implementation scope. |

Supplemental coverage result: 8/8 PASS.

### Docs-governance Prompt

| Prompt ID | Current-safe output | Acceptance conclusion |
| --- | --- | --- |
| CODEX-0040-04-AI-AGENT-SYSTEM-0ed30fc5f0 | `docs/codex/04-ai-agent-system/m_04_ai_agent_system.md` | PASS: governance documentation and traceability evidence recorded in B017 evidence. |

Docs-governance coverage result: 1/1 PASS.

## S07 Fixture Coverage

The S07 fixture set is covered by the B017 contract tests and the stage evidence files.

### expected_events

- `FallbackBlocked`: covered by provider fallback evaluation where silent local-to-cloud fallback returns `SILENT_FALLBACK_FORBIDDEN`.
- `ModelCertificationRecorded`: covered by model certification matrix assertions for Level 1, Level 3, and Level 4 records.

### expected_records

- `ModelRouteSnapshot`: covered with provider type, model id, fallback policy, and privacy boundary assertions.
- `RAGChunk`: covered with source type, visibility, version, and allowed-use assertions.

### expected_errors

- `SILENT_FALLBACK_FORBIDDEN`: covered by provider adapter tests.
- `UNAUTHENTICATED_LOCAL_PROVIDER_EXPOSED`: covered by provider boundary tests.
- `DIRECT_LLM_CALL_FORBIDDEN`: covered by direct provider-call grep and provider adapter boundary tests.
- `LOCAL_MODEL_NOT_CERTIFIED_FOR_AI_KP`: covered by Level 4 AI Keeper certification tests.
- `RAG_VISIBILITY_SCOPE_VIOLATION`: covered by RAG visibility tests.

### pass_criteria

- `provider_adapter_only`: covered by provider boundary tests and direct-call grep.
- `no_silent_fallback`: covered by local-to-cloud fallback denial tests.
- `level4_required_for_ai_kp`: covered by model certification matrix tests.
- `rag_visibility_enforced`: covered by RAG visibility and commit redaction tests.

## Governance Evidence

- Authority Contract remains immutable through private fields/accessors and `new_locked` / `fork_for_child` construction paths.
- Agent Gateway-only AI access is preserved; no business-layer direct LLM/provider call path is accepted by the evidence checks.
- Tool Permission Gate is covered by `agent_tool_gate_cases`.
- Visibility Label Propagation is covered by RAG visibility and commit redaction tests.
- Fact Provenance is covered by agent context/RAG evidence.
- Event Log and State Service boundaries are covered by formal-state write denial evidence.
- V1 Acceptance boundaries are preserved; no source V3/V4/V5/V6 naming is promoted to current implementation semantics.

## Non-applicable Checks

- `pnpm`: N/A. `rg --files -g package.json -g pnpm-lock.yaml -g pnpm-workspace.yaml` found no matching files.
- `docker`: N/A. `rg --files -g Dockerfile -g docker-compose.yml -g docker-compose.yaml` found no matching files.

## Conclusion

S07 acceptance evidence for B017 is complete: 25/25 prompts covered, all S07 fixture categories recorded, and current cargo test/check/clippy results are linked through `S07_TEST_RESULTS.md`.
