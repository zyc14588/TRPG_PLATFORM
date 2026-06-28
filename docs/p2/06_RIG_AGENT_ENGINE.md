# B04 Spec — Rig Agent Engine

## Goal

Introduce a Rust agent/provider orchestration layer built around Rig. This layer enables controlled retrieval workflows and future tool-calling without weakening P2 security. It must not replace `rag_core`, `storage`, RLS, server auth or license/visibility policy.

## Recommended crate

```text
crates/agent_engine
```

Public responsibilities:

- Provider registry and provider policy enforcement.
- Rig client/agent construction behind project-owned traits.
- Retrieval tool wrappers that call policy-guarded service/repository methods.
- Evidence-first workflow orchestration.
- Local deterministic fake engine for tests.

Non-responsibilities:

- Direct SQL queries.
- RLS context management except passing explicit actor/room context to services.
- HTTP route handling.
- UI behavior.
- Final narrative answer generation in P2 default path.

## Rig dependency strategy

Use project style and current Rig ecosystem. Prefer:

- root `rig` facade when feature-gated companion crates are needed;
- `rig-core` when only core provider/agent/completion/embedding abstractions are needed.

Pin explicit versions in `Cargo.toml`; do not use floating dependencies. Feature-gate cloud providers so LocalOnly/no-network test builds do not require secrets.

## Provider registry

A project-owned provider registry should map `PrivacyMode` and room/server policy to allowed provider implementations.

Example conceptual API:

```rust
pub trait AgentProviderRegistry {
    fn completion_provider(&self, policy: &ProviderPolicy) -> Result<Box<dyn CompletionProvider>>;
    fn embedding_provider(&self, policy: &ProviderPolicy) -> Result<Box<dyn Embedder>>;
}
```

Rules:

- `LocalOnly` returns deterministic local/fake providers or local model adapters only.
- `AllowConfiguredCloud` may use configured cloud provider only if server-side config allows it.
- Client request may select policy class but cannot submit API keys.
- Provider errors must not leak secrets.

## Rig tools

All Rig tools must be thin wrappers over safe services:

```text
Rig Agent
  └─ tool: retrieve_room_evidence
        └─ P2 service
             └─ storage repository
                  └─ PostgreSQL RLS / prefilter
```

Forbidden tool behavior:

- direct DB pool access from tool closure;
- reading hidden source content for prompt construction before authorization;
- returning raw DB row;
- bypassing license/visibility prefilter;
- accepting client-supplied role or provider secret.

## P2 agent output contract

Default P2 output is structured evidence, not final GM prose:

```json
{
  "evidence": [],
  "applied_filters": {},
  "provider_metadata": {},
  "agent_trace_id": "...",
  "answer_draft": null
}
```

If experimental answer drafting is added later, it must be behind a disabled-by-default feature flag and must not be called P2 acceptance scope.

## Prompt construction policy

Before any provider prompt:

- candidate evidence must already be authorized;
- evidence budget must be bounded;
- hidden metadata must be stripped;
- citations/content_hash must remain attached;
- denied/pending/cross-room content must be absent, not merely redacted.

## Observability

Agent logs/metrics may include:

- provider kind
- model name/version if non-secret
- latency
- token estimate
- evidence count
- trace id
- error category

Must not include:

- API keys
- raw hidden content
- DB URLs
- bearer/cookie/CSRF tokens

## Required tests

- `rig_local_only_rejects_cloud_completion`
- `rig_local_only_rejects_cloud_embedding`
- `rig_retrieval_tool_uses_policy_guarded_repository`
- `rig_agent_returns_evidence_bundle_not_final_answer`
- `rig_provider_metadata_has_no_secret`
- `rig_tool_does_not_accept_client_role`
- `rig_hidden_denied_pending_not_in_prompt_context`
- `rig_fake_provider_is_deterministic`

## Batch boundary

B04 may add `agent_engine` and integrate with domain/storage service traits. It must not add frontend pages. It should not expose public HTTP routes except compile-only service wiring if unavoidable; server exposure belongs to B05.
