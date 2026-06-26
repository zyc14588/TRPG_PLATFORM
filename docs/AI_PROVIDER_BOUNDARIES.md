# AI Provider Boundaries

## Mandatory Boundaries

All model calls go through `crates/llm_client`. Image calls go through `crates/media_provider::ImageProvider`. Retrieval and embedding orchestration go through `crates/rag_core` and its `Embedder` trait.

Application code must not call OpenAI, Ollama, llama.cpp, image APIs, embedding APIs, rerankers, or other model endpoints directly.

## Privacy Routing

Room privacy modes are hard boundaries:

- `standard`: cloud and local providers may be used according to policy and budget.
- `private_hybrid`: sensitive data should stay local where configured; fallback must not expose protected data unexpectedly.
- `local_only`: chat, embedding, rerank, and image providers must be local. If a local provider is unavailable, the operation fails instead of falling back to cloud.

## Provider Traits

- `LlmProvider::chat_json` is used for schema-constrained model output.
- `EmbeddingProvider` or `rag_core::Embedder` is used for embeddings.
- `ImageProvider` is used for draft-first image generation.

Providers must expose enough metadata to support auditing: provider kind, model name, base URL or endpoint class, token/usage summary, and privacy capability.

## Secrets

API keys must never be committed, returned to clients, written into logs, included in snapshots, or stored in test fixtures. Configuration should reference server-side secret names or environment variables only.

## Agent Output

All Agent outputs that can change game state must pass JSON Schema validation before state application. Dice randomness and mathematical rule resolution must stay deterministic and must not be delegated to LLMs.
