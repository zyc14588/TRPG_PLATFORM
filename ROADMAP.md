# ROADMAP

## Phase 0: Repository Bootstrap

Status: complete in this branch.

Scope: compileable monorepo skeleton, health endpoints, mock provider traits, initial migration, local infrastructure config, and minimal web shell.

## Phase 1: Foundation

Status: Phase 1A complete; Phase 1B verified complete and waiting for human review.

Current: Auth REST, signed access tokens, refresh cookies, refresh rotation, logout revoke, CSRF tests, Room REST, invitations, invite accept, member listing, idempotent create, non-member isolation tests, OpenAPI JSON, route-contract tests, minimal frontend Auth/Room vertical flow, and Phase 1B status docs are implemented.

Verification: P1B backend, frontend, OpenAPI, SQLx, and Compose checks are recorded in `docs/status/PHASE_1B_REPORT.md`.

Next: Human review of Phase 1B. Do not start WebSocket, RAG, Agent, maps, audio, clue graph, or Creator image work in this phase.

## Phase 2+

Realtime, RAG, Agent, UI/maps, Creator images, and hardening remain scoped to their dedicated prompts.
