# Codex Batch 04 — Rig Agent Engine

Compatibility note: this file keeps its legacy name for existing links. It is the B04 Rig Agent Engine prompt. Do not jump to Server API; that is B05.

Start only after Batch 03 is green.

## Read first

- `CODEX_P2_MASTER_PROMPT.md`
- `docs/p2/INDEX.md`
- `docs/p2/00_EXECUTION_RULES.md`
- `docs/p2/02_BATCH_PLAN.md`
- `docs/p2/06_RIG_AGENT_ENGINE.md`
- existing `crates/llm_client`
- existing `crates/rag_core`

## Tasks

1. Implement the minimal Rig-backed `agent_engine` boundary required by `docs/p2/06_RIG_AGENT_ENGINE.md`.
2. Route all model/provider calls through `crates/llm_client`.
3. Route all evidence retrieval through `crates/rag_core` and policy-guarded storage services.
4. Validate every state-changing Agent payload with JSON Schema.
5. Add deterministic no-network tests for provider routing, schema validation, privacy mode blocking, and evidence-first outputs.

## Constraints

- No server route or frontend implementation in this batch.
- No real cloud provider calls in tests.
- Do not bypass storage/RLS or construct prompts from hidden content outside policy gates.
- Do not implement final GM/story answer UX in P2.

## Checks

```powershell
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p agent_engine
cargo test --workspace
cargo sqlx prepare --check --workspace
```

## Completion response

List engine contracts, provider adapters, schema tests, privacy tests, and any deferred provider behavior.
