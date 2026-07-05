# BATCH-021 Prompt Traceability

## Summary

- Declared prompt rows: 25
- Current-safe primary implementation rows in repository maps: 13
- Documentation rows: 1
- Supplemental rows: 11
- Batch fact discrepancy: the user-provided fact says primary count is 0, but `batches/B021.md` and normalized maps identify 13 primary rows. Repository authority was followed.

## Row Results

| Prompt | Role | Evidence | Result |
|---|---|---|---|
| CODEX-0048 / P0006 | documentation | `docs/codex/05-ruleset-coc7/m_05_ruleset_coc7.md` | PASS |
| CODEX-0049 / P0001 | primary | `src/character_combat_san_chase.rs`, `tests/character_combat_san_chase_contract_tests.rs` | PASS |
| CODEX-0050 / P0002 | primary | `src/chase_state_machine.rs`, `tests/chase_state_machine_contract_tests.rs` | PASS |
| CODEX-0051 / P0003 | primary | `src/combat_state_machine.rs`, `tests/combat_state_machine_contract_tests.rs` | PASS |
| CODEX-0052 / P0004 | primary | `src/dice_roll_contract.rs`, `tests/dice_roll_contract_contract_tests.rs` | PASS |
| CODEX-0053 / P0005 | primary | `src/investigation_clue_npc_time.rs`, `tests/investigation_clue_npc_time_contract_tests.rs` | PASS |
| CODEX-0054 / P0007 | primary | `src/rules_coc7.rs`, `tests/rules_coc7_contract_tests.rs` | PASS |
| CODEX-0055 / P0008 | primary | `src/sanity_madness_state_machine.rs`, `tests/sanity_madness_state_machine_contract_tests.rs` | PASS |
| CODEX-0528 / P0009 | supplemental | merged into character/combat/SAN/chase facade constraints | PASS |
| CODEX-0529 / P0010 | supplemental | merged into chase state machine constraints | PASS |
| CODEX-0530 / P0011 | primary | `src/coc7_rules_engine.rs`, `tests/coc7_rules_engine_contract_tests.rs` | PASS |
| CODEX-0531 / P0012 | supplemental | merged into combat state machine constraints | PASS |
| CODEX-0532 / P0013 | supplemental | merged into dice contract constraints | PASS |
| CODEX-0533 / P0014 | primary | `src/san.rs`, `tests/san_contract_tests.rs` | PASS |
| CODEX-0534 / P0015 | primary | `src/npc.rs`, `tests/npc_contract_tests.rs` | PASS |
| CODEX-0535 / P0016 | primary | `src/rule_runtime_coc7_ruleset_pack.rs`, `tests/rule_runtime_coc7_ruleset_pack_contract_tests.rs` | PASS |
| CODEX-0540 / P0017 | primary | `src/rule_runtime_coc7.rs`, `tests/rule_runtime_coc7_contract_tests.rs` | PASS |
| CODEX-0539 / P0018 | supplemental | merged into character/combat/SAN/chase facade constraints | PASS |
| CODEX-0536 / P0019 | supplemental | merged into character/combat/SAN/chase facade constraints | PASS |
| CODEX-0538 / P0020 | primary | `src/coc7_rule_runtime.rs`, `tests/coc7_rule_runtime_contract_tests.rs` | PASS |
| CODEX-0541 / P0021 | supplemental | merged into investigation/clue/NPC/time constraints | PASS |
| CODEX-0537 / P0022 | supplemental | merged into investigation/clue/NPC/time constraints | PASS |
| CODEX-0542 / P0023 | supplemental | merged into SAN constraints | PASS |
| CODEX-0543 / P0024 | supplemental | merged into NPC constraints | PASS |
| CODEX-0544 / P0025 | supplemental | merged into ruleset pack constraints | PASS |

## Governance Checks

- No direct LLM/provider calls were introduced.
- No database or state-service bypass writes were introduced.
- Formal decisions validate `AuthorityContract` and `CommandEnvelope` before `EventStore::append`.
- Event payloads carry `ruleset_id`, decision type, visibility label, and provenance kind from the command.
- Supplemental prompts did not create standalone Rust target outputs.
- No `source-archive/**` path or legacy V3/V4/V5/V6 token was promoted into current module, test, event, subject, workflow, or output names.
