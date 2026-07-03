# BATCH-004 Work Plan

Batch: `BATCH-004-01-foundation`
Stage: `S01` (`stages/s01-foundation-shared-kernel`)
Current batch file: `batches/B004.md`

## Scope

- Declared prompt count: 25.
- Operator-provided recognized primary count: 0.
- Authoritative batch table primary implementation rows: 8.
- Authoritative batch table supplemental requirement rows: 17.
- Scope decision: follow `batches/B004.md` after applying the normalized
  execution map, current-safe output map, and token rewrite table.
- Allowed implementation changes: only the B004 current-safe flat Rust modules,
  their contract tests, and the `lib.rs` module registration needed to expose
  those modules.
- Allowed evidence changes: `evidence/batches/BATCH-004/**`.
- Disallowed changes: later-batch work, migrations, API handlers, NATS
  consumers, provider adapters, database writes, direct model-provider access,
  direct agent state writes, Authority Contract mutation, or promotion of
  historical source names into current code names.

## Prompt Mapping

| Prompt ID | Prompt file | Role | Current-safe target | Allowed change scope | Test responsibility |
|---|---|---|---|---|---|
| CODEX-0164-01-FOUNDATION-15e9dcbbc4 | `codex-prompts/01-foundation/P0024.md` | primary | `src/constitution.rs`; `tests/constitution_contract_tests.rs` | Create constitution governance contract and tests | Required articles, Agent Gateway boundary, event-store write path |
| CODEX-0165-01-FOUNDATION-b8fe9316ba | `codex-prompts/01-foundation/P0027.md` | primary | `src/document_set.rs`; `tests/document_set_contract_tests.rs` | Create foundation document-set contract and tests | Required authority docs, provenance-only historical inputs |
| CODEX-0166-01-FOUNDATION-0d8f080d4d | `codex-prompts/01-foundation/P0023.md` | primary | `src/system_context.rs`; `tests/system_context_contract_tests.rs` | Create system context propagation policy and tests | Visibility/provenance channels, direct provider/write rejection |
| CODEX-0167-01-FOUNDATION-03125d8734 | `codex-prompts/01-foundation/P0029.md` | supplemental | Primary `CODEX-0152`; no new Rust output | Trace only in B004 | Covered by existing `rust_cargo_workspace` tests |
| CODEX-0168-01-FOUNDATION-b45ac02ea6 | `codex-prompts/01-foundation/P0033.md` | supplemental | Primary `CODEX-0164` | Merge into constitution contract | Covered by constitution tests |
| CODEX-0169-01-FOUNDATION-b05b9a90b3 | `codex-prompts/01-foundation/P0032.md` | supplemental | Primary `CODEX-0165` | Merge into document-set contract | Covered by document-set tests |
| CODEX-0170-01-FOUNDATION-306fa4ab16 | `codex-prompts/01-foundation/P0030.md` | supplemental | Primary `CODEX-0161`; no new Rust output | Trace only in B004 | Covered by existing open-source reference matrix tests |
| CODEX-0171-01-FOUNDATION-8a4d752663 | `codex-prompts/01-foundation/P0031.md` | supplemental | Primary `CODEX-0166` | Merge into system-context contract | Covered by system-context tests |
| CODEX-0172-01-FOUNDATION-e9e4f0b4c1 | `codex-prompts/01-foundation/P0034.md` | supplemental | Future primary `CODEX-0222`; no new Rust output | Trace only in B004 | No new test in this batch |
| CODEX-0173-01-FOUNDATION-08cdc4a342 | `codex-prompts/01-foundation/P0035.md` | supplemental | Primary `CODEX-0016`; no new Rust output | Trace only in B004 | Covered by existing cargo workspace tests |
| CODEX-0174-01-FOUNDATION-ba41854b91 | `codex-prompts/01-foundation/P0040.md` | supplemental | Primary `CODEX-0017`; no new Rust output | Trace only in B004 | Covered by existing config model tests |
| CODEX-0175-01-FOUNDATION-e659435928 | `codex-prompts/01-foundation/P0042.md` | supplemental | Primary `CODEX-0018`; no new Rust output | Trace only in B004 | Covered by existing crate ownership tests |
| CODEX-0176-01-FOUNDATION-958c2094d1 | `codex-prompts/01-foundation/P0038.md` | supplemental | Primary `CODEX-0019`; no new Rust output | Trace only in B004 | Covered by existing dependency direction tests |
| CODEX-0177-01-FOUNDATION-4971602f41 | `codex-prompts/01-foundation/P0036.md` | supplemental | Primary `CODEX-0020`; no new Rust output | Trace only in B004 | Covered by existing error model tests |
| CODEX-0178-01-FOUNDATION-561743b9ad | `codex-prompts/01-foundation/P0037.md` | primary | `src/readme.rs`; `tests/readme_contract_tests.rs` | Create README governance contract and tests | Current entry points, provenance boundary |
| CODEX-0179-01-FOUNDATION-dc81653f92 | `codex-prompts/01-foundation/P0041.md` | supplemental | Primary `CODEX-0021`; no new Rust output | Trace only in B004 | Covered by existing rust coding model tests |
| CODEX-0180-01-FOUNDATION-b6f4482b91 | `codex-prompts/01-foundation/P0039.md` | supplemental | Primary `CODEX-0022`; no new Rust output | Trace only in B004 | Covered by existing shared kernel tests |
| CODEX-0181-01-FOUNDATION-5c3ee7877d | `codex-prompts/01-foundation/P0043.md` | supplemental | Primary `CODEX-0178` | Merge into README contract | Covered by readme tests |
| CODEX-0182-01-FOUNDATION-1226a36865 | `codex-prompts/01-foundation/P0044.md` | supplemental | Primary `CODEX-0021`; no new Rust output | Trace only in B004 | Covered by existing rust coding model tests |
| CODEX-0183-01-FOUNDATION-c715e94c4a | `codex-prompts/01-foundation/P0045.md` | supplemental | Primary `CODEX-0022`; no new Rust output | Trace only in B004 | Covered by existing shared kernel tests |
| CODEX-0184-01-FOUNDATION-da009f8e1c | `codex-prompts/01-foundation/P0046.md` | primary | `src/workspace_and_governance.rs`; `tests/workspace_and_governance_contract_tests.rs` | Create shared governance contract API and tests | Command fields, bypass rejection, event-store append |
| CODEX-0185-01-FOUNDATION-2d0624102a | `codex-prompts/01-foundation/P0047.md` | supplemental | Primary `CODEX-0164` | Merge into constitution contract | Covered by constitution tests |
| CODEX-0186-01-FOUNDATION-337c5fa749 | `codex-prompts/01-foundation/P0048.md` | primary | `src/cargo_workspace_impl.rs`; `tests/cargo_workspace_impl_contract_tests.rs` | Create cargo workspace landing contract and tests | Resolver, shared-kernel member, governed review event |
| CODEX-0187-01-FOUNDATION-5869480a21 | `codex-prompts/01-foundation/P0049.md` | primary | `src/constitution_impl.rs`; `tests/constitution_impl_contract_tests.rs` | Create constitution landing contract and tests | Complete constitution checklist, governed review event |
| CODEX-0188-01-FOUNDATION-6619678123 | `codex-prompts/01-foundation/P0050.md` | primary | `src/document_set_impl.rs`; `tests/document_set_impl_contract_tests.rs` | Create document-set landing contract and tests | Complete document set, governed review event |

## Check Order

1. B004 current-safe target existence assertions.
2. Minimal Rust check: `cargo test -p trpg-shared-kernel --all-features`.
3. S01 checks: `cargo fmt --all -- --check`,
   `cargo clippy -p trpg-shared-kernel --all-targets --all-features -- -D warnings`,
   and `cargo test --workspace --all-features`.
4. Static boundary scans for template names, untyped JSON, direct provider
   strings, and historical tokens in Rust src/tests.
5. S01 fixture-declared token assertions.
6. Evidence write-up under `evidence/batches/BATCH-004/`.
