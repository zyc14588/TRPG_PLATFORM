# BATCH-018 Strict Repair Plan

Authority source: `batches/B018.md`.

Explicit scope correction: ignore `batch-prompts/start/B018.md` primary=0. The active batch has 25 prompt rows, including 6 `primary-implementation` rows.

## Allowed Implementation

Create only the current-safe Rust targets and matching contract tests listed by `batches/B018.md`:

- `agent_runtime::ai_agent`
- `agent_runtime::readme`
- `agent_runtime::agent_pack_sdk`
- `agent_runtime::plugin_ruleset_agent_pack_sdk`
- `agent_runtime::agent_runtime_impl`
- `agent_runtime::evaluation_golden_scenario_impl`

## Non-Goals

- No migrations.
- No API handlers.
- No NATS subjects.
- No workflow expansion outside existing EventStore boundary.
- No direct OpenAI, Ollama, llama.cpp, HTTP, SDK, or provider access outside the existing model provider boundary.
- No supplemental prompt implementation beyond its primary owner.

## Governance Constraints

- Authority Contract remains immutable; mismatch returns existing authority errors.
- Formal writes use existing `CommandEnvelope -> commit_agent_decision -> EventStore.append` only.
- Tool Permission Gate uses existing `evaluate_agent_tool_request`.
- Visibility Label and Fact Provenance are preserved through command/event and replay checks.
- Player-visible text is redacted through existing restricted-token redaction.
- Local/cloud provider fallback remains governed by existing provider adapter policy.

## Test Plan

- `cargo fmt --all -- --check`
- `cargo clippy -p trpg-agent-runtime --all-targets --all-features --target-dir target\b018-check --jobs 1 -- -D warnings`
- `cargo check -p trpg-agent-runtime --target-dir target\b018-check`
- `cargo test -p trpg-agent-runtime --all-features --target-dir target\b018-check --jobs 1`
- Six B018 primary contract test targets.
- Existing S07 fixture-bearing target: `cargo test -p trpg-agent-runtime --test batch_017_agent_runtime_contract_tests --target-dir target\b018-check --jobs 1`
- Fixture JSON fence parse with PowerShell `Get-Content -Encoding UTF8` and `ConvertFrom-Json`.
- Static scans for direct provider calls, non-EventStore writes, unsafe historical names, and private-token leakage assertions.

## pnpm/docker

No root `package.json`, `pnpm-lock.yaml`, `Dockerfile`, or `docker-compose*.yml/yaml` exists. pnpm/docker checks are not applicable for this Rust-only batch slice.
