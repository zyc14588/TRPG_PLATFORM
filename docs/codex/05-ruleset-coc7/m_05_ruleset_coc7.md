# S05 COC7 Ruleset Current-Safe Module

Status: BATCH-021 strict governance implementation.

## Current-Safe Output

- Crate: `crates/trpg-ruleset-coc7`
- Ruleset id: `coc7`
- Authority path: `AuthorityContract` + `CommandEnvelope` + `EventStore`
- Formal writes: all recorded decisions use shared-kernel governed command validation before event append.
- AI/model boundary: this ruleset crate contains no direct OpenAI, Ollama, llama.cpp, model-provider, or database calls.

## Implemented Prompt Coverage

| Prompt | Current-safe module |
|---|---|
| CODEX-0049 | `character_combat_san_chase` |
| CODEX-0050 | `chase_state_machine` |
| CODEX-0051 | `combat_state_machine` |
| CODEX-0052 | `dice_roll_contract` |
| CODEX-0053 | `investigation_clue_npc_time` |
| CODEX-0054 | `rules_coc7` |
| CODEX-0055 | `sanity_madness_state_machine` |
| CODEX-0530 | `coc7_rules_engine` |
| CODEX-0533 | `san` |
| CODEX-0534 | `npc` |
| CODEX-0535 | `rule_runtime_coc7_ruleset_pack` |
| CODEX-0540 | `rule_runtime_coc7` |
| CODEX-0538 | `coc7_rule_runtime` |

Supplemental prompts in B021 are merged into these primary module constraints and tests; they do not define independent Rust outputs.

## Verification

- `cargo check -p trpg-ruleset-coc7`
- `cargo test -p trpg-ruleset-coc7`
- `cargo fmt --all -- --check`
- `cargo test --workspace`

Batch evidence is recorded under `evidence/batches/BATCH-021/`.
