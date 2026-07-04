# S07 Test Results - BATCH-017

Stage: S07 - Agent Runtime, Provider Adapter, Model Certification, RAG Visibility
Batch: BATCH-017-04-ai-agent-system
Evidence date: 2026-07-05
Repair scope: evidence only. No implementation code changed for this report.

## Command Results

| Command | Result | Evidence |
| --- | --- | --- |
| `cargo test -p trpg-agent-runtime --test batch_017_agent_runtime_contract_tests --all-features` | PASS | 12/12 B017 contract tests passed. |
| `cargo test -p trpg-agent-runtime --all-features` | PASS | Agent runtime crate tests passed, including the 12 B017 contract tests. |
| `cargo test -p trpg-domain-core --all-features` | PASS | Domain core tests passed, including compile-fail doctests proving external `DomainAuthorityContract` field mutation is rejected. |
| `cargo check --workspace --all-targets --all-features` | PASS | Workspace all-target/all-feature check passed. |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS | Workspace clippy passed with warnings denied. |
| `cargo test --workspace --all-features` | PASS | Workspace all-feature tests passed. |
| `cargo fmt --all -- --check` | PASS | Formatting check passed. |
| `git diff --check` | PASS | Whitespace check passed; only existing LF/CRLF warnings were reported. |

Observed non-failing cargo note: the workspace emitted a warning that `C:\Users\zyc14588` could not be canonicalized. It did not fail any command.

## B017 Contract Tests

The current B017 contract suite contains 12 tests and passed:

- S07 provider/model/RAG fixture assertions.
- Agent runtime commit-path redaction for restricted fixture tokens.
- Agent Gateway-only AI access boundaries.
- Tool Permission Gate cases.
- Local model certification Level 4 gate for AI Keeper.
- Provider adapter no-silent-fallback and production local-provider exposure denial.
- Primary wrapper module entrypoint coverage for prompt IDs.

## S07 Fixture Assertions

Fixture files covered by the B017 contract suite:

- `fixtures/stages/S07_stage_acceptance_fixture.v1.json.md`
- `fixtures/stages/detailed/S07_provider_rag_model_cert_expected.current.json.md`
- `fixtures/provider/model_certification_matrix.v1.json.md`
- `fixtures/provider/agent_tool_gate_cases.v1.json.md`
- `fixtures/provider/rag_snapshot_cases.v1.json.md`

### expected_events

| Fixture expectation | Result | Evidence |
| --- | --- | --- |
| `FallbackBlocked` | PASS | `evaluate_cloud_fallback` rejects local-to-cloud silent fallback with `SILENT_FALLBACK_FORBIDDEN`. |
| `ModelCertificationRecorded` | PASS | Model certification matrix records and asserts certification levels before AI Keeper eligibility checks. |

### expected_records

| Fixture expectation | Result | Evidence |
| --- | --- | --- |
| `ModelRouteSnapshot` | PASS | Provider type, model id, fallback policy, and privacy boundary are asserted in provider adapter tests. |
| `RAGChunk` | PASS | Source type, visibility, version, and allowed-use metadata are asserted in RAG visibility tests. |

### expected_errors

| Fixture expectation | Result | Evidence |
| --- | --- | --- |
| `SILENT_FALLBACK_FORBIDDEN` | PASS | Silent local-to-cloud fallback is denied. |
| `UNAUTHENTICATED_LOCAL_PROVIDER_EXPOSED` | PASS | Production exposure of unauthenticated local provider is denied. |
| `DIRECT_LLM_CALL_FORBIDDEN` | PASS | Direct provider/LLM access outside the adapter boundary is denied by tests and grep evidence. |
| `LOCAL_MODEL_NOT_CERTIFIED_FOR_AI_KP` | PASS | Non-Level-4 local models cannot serve AI Keeper Orchestrator. |
| `RAG_VISIBILITY_SCOPE_VIOLATION` | PASS | RAG visibility scope violations are rejected. |

### pass_criteria

| Fixture criterion | Result | Evidence |
| --- | --- | --- |
| `provider_adapter_only` | PASS | Provider boundary tests and direct-call checks confirm adapter-only access. |
| `no_silent_fallback` | PASS | Local-to-cloud silent fallback is rejected. |
| `level4_required_for_ai_kp` | PASS | AI Keeper requires Level 4 model certification. |
| `rag_visibility_enforced` | PASS | RAG chunks and committed decisions enforce visibility labels and restricted-token redaction. |

## Prompt Coverage Result

- B017 prompt coverage: 25/25 PASS.
- Role conclusion: 16 primary/8 supplemental/1 docs-governance PASS.
- Primary prompts: 16/16 PASS.
- Supplemental prompts: 8/8 PASS.
- Docs-governance prompts: 1/1 PASS.

## pnpm and Docker

- `pnpm`: N/A. No `package.json`, `pnpm-lock.yaml`, or `pnpm-workspace.yaml` files are present.
- `docker`: N/A. No `Dockerfile`, `docker-compose.yml`, or `docker-compose.yaml` files are present.

## Conclusion

Current S07/B017 cargo test/check/clippy evidence is PASS, with pnpm and docker explicitly marked N/A for this repository.
