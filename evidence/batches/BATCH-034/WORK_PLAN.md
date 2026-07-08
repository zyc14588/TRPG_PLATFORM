# BATCH-034 Work Plan

## Scope

- Stage: S09 Platform Infrastructure
- Batch: BATCH-034-08-platform-infrastructure
- Batch file: `batches/B034.md`
- Prompt count: 2
- Current-safe maps applied:
  - `docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`
  - `docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md`
  - `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`

## Prompt Mapping

| Prompt ID | Role | Current-safe target | Allowed changes | Test responsibility |
|---|---|---|---|---|
| `CODEX-0791-08-PLATFORM-INFRASTRUCTURE-8ec2816185` | supplemental-requirement | `platform_infrastructure::reliability_performance`; output remains `codex-prompts/08-platform-infrastructure/P0076.md` | No Rust src/test changes. Only supplemental merge instructions to primary `CODEX-0728-08-PLATFORM-INFRASTRUCTURE-07cc301db8` if needed. | Verify supplemental boundary; do not run reliability implementation changes in this batch. |
| `CODEX-0792-08-PLATFORM-INFRASTRUCTURE-ef0ee5cd23` | primary-implementation | `crates/trpg-platform/src/security_privacy_copyright.rs`; `crates/trpg-platform/tests/security_privacy_copyright_contract_tests.rs` | Add current-safe flat Rust module, expose it from `lib.rs`, add focused contract tests. No migrations/API/NATS subjects unless already required by this module slice. | Run module tests and S09 trpg-platform checks. |

## Implementation Slice

1. Add `security_privacy_copyright` flat module using existing `trpg_shared_kernel::CommandEnvelope` and `EventStore`.
2. Enforce policy default-deny inputs, restricted visibility export denial, redaction for observability, authority/write-path/idempotency/expected-version through shared kernel append.
3. Add contract tests for authority mismatch, visibility/provenance replay, policy denial, expected version, duplicate idempotency, deletion request eventing, and current-safe naming.
4. Record evidence under `evidence/batches/BATCH-034/`.

## Out Of Scope

- No changes to BATCH-031/BATCH-032/BATCH-033 outputs.
- No source-archive derived names in current modules, events, metrics, or tests.
- No direct LLM calls, direct DB writes, Authority Contract mutation, or visibility policy weakening.
