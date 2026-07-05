# BATCH-021-05-ruleset-coc7 Strict Governance Final Plan

Baseline date: 2026-07-05

## Scope Guard

- Batch file: `batches/B021.md`
- Stage: `stages/s05-ruleset-coc7-engine`
- Current-safe target crate: `crates/trpg-ruleset-coc7`
- `source-archive/**` is provenance only. No V3/V4/V5/V6 historical names may become current Rust modules, migrations, events, metrics, tests, workflows, or output names.
- Shared kernel primitives must be reused for authority, command validation, event append, visibility, and fact provenance.
- No direct OpenAI/Ollama/llama.cpp/provider calls, no database writes, no agent direct state writes, no Authority Contract mutation.

## Prompt Mapping

| Prompt | Role | Current-safe target | Allowed change | Test responsibility |
|---|---|---|---|---|
| CODEX-0048 / P0006 | documentation | `docs/codex/05-ruleset-coc7/m_05_ruleset_coc7.md` | Traceability only | Covered by evidence traceability |
| CODEX-0049 / P0001 | primary | `crates/trpg-ruleset-coc7/src/character_combat_san_chase.rs` | Implement character/combat/SAN/chase shared facade | `tests/character_combat_san_chase_contract_tests.rs` |
| CODEX-0050 / P0002 | primary | `crates/trpg-ruleset-coc7/src/chase_state_machine.rs` | Implement governed chase transitions | `tests/chase_state_machine_contract_tests.rs` |
| CODEX-0051 / P0003 | primary | `crates/trpg-ruleset-coc7/src/combat_state_machine.rs` | Implement governed combat transitions | `tests/combat_state_machine_contract_tests.rs` |
| CODEX-0052 / P0004 | primary | `crates/trpg-ruleset-coc7/src/dice_roll_contract.rs` | Implement server dice contract and success levels | `tests/dice_roll_contract_contract_tests.rs` |
| CODEX-0053 / P0005 | primary | `crates/trpg-ruleset-coc7/src/investigation_clue_npc_time.rs` | Implement clue fail-forward, NPC visibility, time decisions | `tests/investigation_clue_npc_time_contract_tests.rs` |
| CODEX-0054 / P0007 | primary | `crates/trpg-ruleset-coc7/src/rules_coc7.rs` | Implement ruleset metadata and dispatch guard | `tests/rules_coc7_contract_tests.rs` |
| CODEX-0055 / P0008 | primary | `crates/trpg-ruleset-coc7/src/sanity_madness_state_machine.rs` | Implement SAN loss and madness transitions | `tests/sanity_madness_state_machine_contract_tests.rs` |
| CODEX-0528 / P0009 | supplemental | merged into CODEX-0049 | Supplemental constraints only | Covered by character facade tests |
| CODEX-0529 / P0010 | supplemental | merged into CODEX-0050 | Supplemental constraints only | Covered by chase tests |
| CODEX-0530 / P0011 | primary | `crates/trpg-ruleset-coc7/src/coc7_rules_engine.rs` | Implement governed rules engine entry point | `tests/coc7_rules_engine_contract_tests.rs` |
| CODEX-0531 / P0012 | supplemental | merged into CODEX-0051 | Supplemental constraints only | Covered by combat tests |
| CODEX-0532 / P0013 | supplemental | merged into CODEX-0052 | Supplemental constraints only | Covered by dice tests |
| CODEX-0533 / P0014 | primary | `crates/trpg-ruleset-coc7/src/san.rs` | Implement SAN helpers over state machine | `tests/san_contract_tests.rs` |
| CODEX-0534 / P0015 | primary | `crates/trpg-ruleset-coc7/src/npc.rs` | Implement NPC secret visibility helpers | `tests/npc_contract_tests.rs` |
| CODEX-0535 / P0016 | primary | `crates/trpg-ruleset-coc7/src/rule_runtime_coc7_ruleset_pack.rs` | Implement ruleset pack contract | `tests/rule_runtime_coc7_ruleset_pack_contract_tests.rs` |
| CODEX-0540 / P0017 | primary | `crates/trpg-ruleset-coc7/src/rule_runtime_coc7.rs` | Implement runtime decision facade | `tests/rule_runtime_coc7_contract_tests.rs` |
| CODEX-0539 / P0018 | supplemental | merged into CODEX-0049 | Supplemental constraints only | Covered by character facade tests |
| CODEX-0536 / P0019 | supplemental | merged into CODEX-0049 | Supplemental constraints only | Covered by character facade tests |
| CODEX-0538 / P0020 | primary | `crates/trpg-ruleset-coc7/src/coc7_rule_runtime.rs` | Implement runtime governance guard | `tests/coc7_rule_runtime_contract_tests.rs` |
| CODEX-0541 / P0021 | supplemental | merged into CODEX-0053 | Supplemental constraints only | Covered by investigation tests |
| CODEX-0537 / P0022 | supplemental | merged into CODEX-0053 | Supplemental constraints only | Covered by investigation tests |
| CODEX-0542 / P0023 | supplemental | merged into CODEX-0533 | Supplemental constraints only | Covered by SAN tests |
| CODEX-0543 / P0024 | supplemental | merged into CODEX-0534 | Supplemental constraints only | Covered by NPC tests |
| CODEX-0544 / P0025 | supplemental | merged into CODEX-0535 | Supplemental constraints only | Covered by ruleset pack tests |

## Execution Notes

- `batches/B021.md` declares 25 prompts with primary prompts present, while the user supplied current batch fact says primary count is 0. This plan follows the repository batch file and normalized maps.
- Implementation will stay inside the new COC7 ruleset crate plus the workspace manifest and batch evidence.
- Minimal checks will target `trpg-ruleset-coc7`; stage check will use the same crate-level test surface because S05 has no broader runnable script in the loaded stage files.
