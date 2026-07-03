# BATCH-007 Work Plan

Batch: `BATCH-007-02-domain-core`

Stage: `S02`

Authority inputs read:

- `AGENTS.md`
- `CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`
- `SOURCE_BUNDLE_INTEGRATION_GUIDE.md`
- `docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md`
- `docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`
- `docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md`
- `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`
- `V1_ACCEPTANCE_EVIDENCE_MATRIX.md`
- `PER_STAGE_FIXTURE_EXPANSION_PLAN.md`
- `stages/s02-domain-core-authority-event-model/*`
- `batches/B007.md`
- `docs/codex/02-domain-core/*`
- B007 per-file prompts listed below

Scope note:

The user-supplied batch fact said primary prompt count was `0`, but
`batches/B007.md`, `per-file-prompt-manifest.md`, and current-safe maps
identify this batch as 25 prompts: 13 primary implementation prompts, 11
supplemental prompts, and 1 docs-governance prompt. Current-safe repository
materials were used.

## Prompt Map

| Prompt ID | Role | Current-safe target | Allowed change | Test responsibility |
|---|---|---|---|---|
| `CODEX-0023-02-DOMAIN-CORE-afd26a0bfd` | docs-governance | `docs/codex/02-domain-core/m_02_domain_core.md` | Markdown trace only | Link batch coverage |
| `CODEX-0024-02-DOMAIN-CORE-f28763f1b2` | primary | `crates/trpg-domain-core/src/authority_contract_guard.rs` | Rust + contract test | Authority guard allow/deny |
| `CODEX-0025-02-DOMAIN-CORE-d6e83289f7` | primary | `crates/trpg-domain-core/src/command_cqrs_idempotency.rs` | Rust + contract test | idempotency + expected_version |
| `CODEX-0026-02-DOMAIN-CORE-19aeeb927d` | primary | `crates/trpg-domain-core/src/decision_record_model.rs` | Rust + contract test | required audit fields |
| `CODEX-0027-02-DOMAIN-CORE-dd3272feaf` | primary | `crates/trpg-domain-core/src/domain_entities_value_objects.rs` | Rust + contract test | confirmed fact source |
| `CODEX-0028-02-DOMAIN-CORE-3acc117855` | primary | `crates/trpg-domain-core/src/domain_policy_hooks.rs` | Rust + contract test | policy default deny |
| `CODEX-0029-02-DOMAIN-CORE-37d1d366c5` | primary | `crates/trpg-domain-core/src/fork_canon_lineage.rs` | Rust + contract test | fork parent unchanged |
| `CODEX-0030-02-DOMAIN-CORE-b694d9c49d` | primary | `crates/trpg-domain-core/src/visibility_fact_provenance.rs` | Rust + contract test | redaction + provenance |
| `CODEX-0237-02-DOMAIN-CORE-b5d89d5c05` | primary | `crates/trpg-domain-core/src/adr_0003_authority_contract_authority_contract.rs` | Rust + contract test | ADR invariant wrapper |
| `CODEX-0238-02-DOMAIN-CORE-84c2536bcd` | supplemental | `codex-prompts/02-domain-core/P0010.md` | Markdown only | merged to authority guard |
| `CODEX-0239-02-DOMAIN-CORE-626c99ebbb` | primary | `crates/trpg-domain-core/src/command_authority_visibility.rs` | Rust + contract test | authority + visibility |
| `CODEX-0240-02-DOMAIN-CORE-29037d1e55` | supplemental | `codex-prompts/02-domain-core/P0012.md` | Markdown only | merged to idempotency |
| `CODEX-0241-02-DOMAIN-CORE-72131fafdf` | supplemental | `codex-prompts/02-domain-core/P0013.md` | Markdown only | merged to decision record |
| `CODEX-0242-02-DOMAIN-CORE-a2ed8f32e6` | supplemental | `codex-prompts/02-domain-core/P0014.md` | Markdown only | merged to value objects |
| `CODEX-0243-02-DOMAIN-CORE-79a1b0d106` | supplemental | `codex-prompts/02-domain-core/P0015.md` | Markdown only | merged to policy hooks |
| `CODEX-0244-02-DOMAIN-CORE-f4b75ec825` | supplemental | `codex-prompts/02-domain-core/P0016.md` | Markdown only | merged to fork lineage |
| `CODEX-0245-02-DOMAIN-CORE-34eeb364aa` | primary | `crates/trpg-domain-core/src/authority_contract.rs` | Rust + contract test | locked/fork-only contract |
| `CODEX-0246-02-DOMAIN-CORE-87c3f50d0f` | primary | `crates/trpg-domain-core/src/command_cqrs.rs` | Rust + contract test | formal event append |
| `CODEX-0247-02-DOMAIN-CORE-3837e9b57c` | primary | `crates/trpg-domain-core/src/ddd.rs` | Rust + contract test | error/source boundary |
| `CODEX-0248-02-DOMAIN-CORE-c3a01b2873` | primary | `crates/trpg-domain-core/src/event_sourcing_snapshot_projection.rs` | Rust + contract test | replay/rebuild snapshot |
| `CODEX-0249-02-DOMAIN-CORE-08e97c524e` | supplemental | `codex-prompts/02-domain-core/P0021.md` | Markdown only | merged to visibility |
| `CODEX-0250-02-DOMAIN-CORE-20bde1eea0` | supplemental | `codex-prompts/02-domain-core/P0023.md` | Markdown only | merged to authority guard |
| `CODEX-0251-02-DOMAIN-CORE-0f46c363c7` | supplemental | `codex-prompts/02-domain-core/P0022.md` | Markdown only | merged to idempotency |
| `CODEX-0252-02-DOMAIN-CORE-e1fbc903f4` | supplemental | `codex-prompts/02-domain-core/P0024.md` | Markdown only | merged to visibility |
| `CODEX-0253-02-DOMAIN-CORE-efa750909e` | supplemental | `codex-prompts/02-domain-core/P0029.md` | Markdown only | merged to authority contract |

## Minimal Slice

- Create `trpg-domain-core`.
- Reuse `trpg-shared-kernel` types for command, event, visibility, actor,
  and fact provenance.
- Implement pure domain logic only; no SQLx/API/NATS/provider work in this
  batch.
