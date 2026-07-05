# BATCH-022-05-ruleset-coc7 Strict Governance Final Plan

Baseline date: 2026-07-05

## Scope Guard

- Batch file: `batches/B022.md`
- Stage: `stages/s05-ruleset-coc7-engine`
- Current-safe target crate: `crates/trpg-ruleset-coc7`
- `source-archive/**` is provenance only. No historical V3/V4/V5/V6 names, hashes, or source-path fragments may become current modules, migrations, events, metrics, tests, workflows, or output names.
- B022 start prompt says primary count is 0, but `batches/B022.md`, the category manifest, and the normalized maps identify three current-safe primary implementation rows: CODEX-0550, CODEX-0554, and CODEX-0557. Repository maps are followed and the discrepancy is recorded.
- All formal writes must stay on `CommandEnvelope -> AuthorityContract::validate_command -> EventStore::append`.
- No direct OpenAI/Ollama/llama.cpp/provider calls, no database writes, no agent direct state writes, no Authority Contract mutation.

## Prompt Mapping

| Prompt | Role | Current-safe target | Allowed change | Test responsibility |
|---|---|---|---|---|
| CODEX-0545 / P0026 | supplemental | merge into CODEX-0049 character/combat/SAN/chase | Supplemental constraints only | Existing character facade tests |
| CODEX-0546 / P0028 | supplemental | merge into CODEX-0050 chase state machine | Supplemental constraints only | Existing chase tests |
| CODEX-0547 / P0027 | supplemental | merge into CODEX-0051 combat state machine | Supplemental constraints only | Existing combat tests |
| CODEX-0548 / P0030 | supplemental | merge into CODEX-0052 dice contract | Supplemental constraints only | Existing dice tests |
| CODEX-0549 / P0029 | supplemental | merge into CODEX-0053 investigation/clue/NPC/time | Supplemental constraints only | Existing investigation tests |
| CODEX-0550 / P0032 | primary | `crates/trpg-ruleset-coc7/src/coc7.rs` | Implement COC7 strict governance entry contract | `tests/coc7_contract_tests.rs` |
| CODEX-0551 / P0033 | supplemental | merge into CODEX-0538 COC7 rule runtime | Supplemental constraints only | Existing `coc7_rule_runtime` tests |
| CODEX-0552 / P0031 | supplemental | merge into CODEX-0533 SAN | Supplemental constraints only | Existing SAN tests |
| CODEX-0553 / P0034 | supplemental | merge into CODEX-0053 investigation/clue/NPC/time | Supplemental constraints only | Existing investigation tests |
| CODEX-0554 / P0035 | primary | `crates/trpg-ruleset-coc7/src/readme.rs` | Implement README-derived strict governance contract | `tests/readme_contract_tests.rs` |
| CODEX-0555 / P0036 | supplemental | merge into CODEX-0054 rules_coc7 | Supplemental constraints only | Existing rules_coc7 tests |
| CODEX-0556 / P0037 | supplemental | merge into CODEX-0055 sanity madness state machine | Supplemental constraints only | Existing sanity madness tests |
| CODEX-0557 / P0038 | primary | `crates/trpg-ruleset-coc7/src/ruleset_pack_sdk.rs` | Implement ruleset pack SDK contract | `tests/ruleset_pack_sdk_contract_tests.rs` |
| CODEX-0558 / P0039 | supplemental | merge into CODEX-0557 ruleset pack SDK | Supplemental constraints only | `ruleset_pack_sdk` tests |
| CODEX-0559 / P0040 | supplemental | merge into CODEX-0540 rule runtime COC7 | Supplemental constraints only | Existing rule runtime tests |
| CODEX-0560 / P0041 | supplemental | merge into CODEX-0049 character/combat/SAN/chase | Supplemental constraints only | Existing character facade tests |
| CODEX-0561 / P0044 | traceability | `docs/codex/05-ruleset-coc7/source_processing_record_docs_domain_character_combat_san_chase.md` | Markdown provenance record | Prompt traceability evidence |
| CODEX-0562 / P0042 | traceability | `docs/codex/05-ruleset-coc7/source_processing_record_docs_domain_investigation_clue_npc_time.md` | Markdown provenance record | Prompt traceability evidence |
| CODEX-0563 / P0043 | traceability | `docs/codex/05-ruleset-coc7/source_processing_record_docs_domain_rule_runtime_coc7.md` | Markdown provenance record | Prompt traceability evidence |
| CODEX-0564 / P0054 | traceability | `docs/codex/05-ruleset-coc7/source_processing_record_docs_implementation_05_ruleset_coc7_character_combat_san_chase.md` | Markdown provenance record | Prompt traceability evidence |
| CODEX-0565 / P0055 | traceability | `docs/codex/05-ruleset-coc7/source_processing_record_docs_implementation_05_ruleset_coc7_chase_state_machine.md` | Markdown provenance record | Prompt traceability evidence |
| CODEX-0566 / P0053 | traceability | `docs/codex/05-ruleset-coc7/source_processing_record_docs_implementation_05_ruleset_coc7_combat_state_machine.md` | Markdown provenance record | Prompt traceability evidence |
| CODEX-0567 / P0048 | traceability | `docs/codex/05-ruleset-coc7/source_processing_record_docs_implementation_05_ruleset_coc7_dice_roll_contract.md` | Markdown provenance record | Prompt traceability evidence |
| CODEX-0568 / P0046 | traceability | `docs/codex/05-ruleset-coc7/source_processing_record_docs_implementation_05_ruleset_coc7_investigation_clue_npc_time.md` | Markdown provenance record | Prompt traceability evidence |
| CODEX-0569 / P0050 | traceability | `docs/codex/05-ruleset-coc7/source_processing_record_docs_implementation_05_ruleset_coc7_readme.md` | Markdown provenance record | Prompt traceability evidence |

## Test Plan

1. Minimal related checks:
   - `cargo test -p trpg-ruleset-coc7 coc7`
   - `cargo test -p trpg-ruleset-coc7 readme`
   - `cargo test -p trpg-ruleset-coc7 ruleset_pack_sdk`
2. Stage checks:
   - `cargo test -p trpg-ruleset-coc7 --all-features`
   - `cargo test -p trpg-ruleset-coc7 dice`
   - `cargo test -p trpg-ruleset-coc7 sanity`
   - `cargo fmt --all -- --check`

## Execution Notes

- No migrations, API handlers, WebSocket contracts, NATS subjects, providers, or database paths are created in this batch.
- Supplemental rows are recorded as merged constraints into their primary owner modules; they do not create standalone Rust outputs.
- Traceability rows create only Markdown records.
