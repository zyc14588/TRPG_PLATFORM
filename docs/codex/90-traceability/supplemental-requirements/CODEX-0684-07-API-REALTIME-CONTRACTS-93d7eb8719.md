# Supplemental Requirement Merge Instruction

Prompt ID: `CODEX-0684-07-API-REALTIME-CONTRACTS-93d7eb8719`
Primary Prompt: `CODEX-0067-07-API-REALTIME-CONTRACTS-1ccbeea1df`
Batch: `BATCH-029-07-api-realtime-contracts`
Target module: `api_realtime_contracts::external_provider_contracts`
Source prompt: `codex-prompts/07-api-realtime-contracts/P0010.md`

## Boundary

This supplemental prompt is not an independent Rust source, test, migration, API handler, NATS subject, or formal state write owner.

## Merge Instruction

Merge these constraints into the primary external provider contract work:

- Allow AI/provider access only through the Agent Gateway governed path.
- Deny direct model provider calls and runtime adapter bypasses from business, API, KP service, rules, or frontend layers.
- Require audit fields for correlation ID, causation ID, visibility, fact provenance, and Authority Contract version.
- Keep local and cloud provider boundaries explicit; do not allow silent cross-boundary fallback.
- Keep all provider contract names current-safe and free of historical version tokens.
