# Phase 1 Report

Status: accepted in local verification; waiting for human review and commit.

This is the Phase 1 roll-up report. Detailed batch reports remain in:

- `docs/status/PHASE_1A_REPORT.md`
- `docs/status/PHASE_1B_REPORT.md`

## Scope Completed

- Development-only identity flow with Magic Link and mock OIDC ports.
- Explicit `DevelopmentAuthProvider`; production defaults do not expose development login tokens.
- Access token, rotating refresh session, logout revoke, CSRF protection, and `/api/me`.
- Room creation, listing, lookup, invitations, invite accept, member listing, and idempotent room create replay.
- Application ABAC plus PostgreSQL RLS for room/member/invite access.
- Audit rows for login, room create, invite create, invite accept, and denied access.
- Safe frontend room DTO parsing and rejection of KP-only fields.
- Static OpenAPI contract served at `/openapi.json` and route-contract tested.
- Thin frontend auth/room flow and API-client E2E against a real Rust API process.

## Out Of Scope

No Phase 1 work implemented RAG, Agent behavior, WebSocket replay, tactical maps, TTS, Creator images, commercial rule text, real email delivery, real OIDC providers, OpenAI/Ollama calls, or paid external APIs.

## Acceptance Matrix

| Check | Result |
|---|---|
| No Phase 0 repeat | Passed. Phase 0 initial migration was not edited; Compose was not rebuilt. |
| No scope creep | Passed. Out-of-scope systems stayed untouched beyond existing stubs/docs. |
| CI isolation | Passed. Verification used local Postgres and mock/development auth only. |
| Auth mode | Passed. Development auth is explicit; missing `TRPG_AUTH_MODE` defaults to production. |
| ABAC plus RLS | Passed. API cross-room denial and direct DB/RLS denial are both tested. |
| Safe frontend DTOs | Passed. Frontend rejects KP-only fields and does not consume raw DB entities. |
| OpenAPI | Passed. P1 auth and room routes are documented and route-contract tested. |
| Migrations | Passed. P1 migrations are additive; Phase 0 migration stayed unchanged. |
| Tests | Passed. Unit, storage integration, RLS, API security, frontend, and E2E tests cover the vertical flow. |
| Dependencies | Passed. New dependency usage is limited to auth/hash basics; no SMTP, real OIDC SDK, LLM SDK, or UI component library. |
| Stop points | Passed. A1/A2/A3/B1/B2/B3 each has a runnable stop point recorded in `docs/plan/PHASE_1_PLAN.md`. |

## Latest Verification

Last local acceptance pass used an isolated temporary `pgvector/pgvector:pg17` PostgreSQL container for migration, SQLx, and DB-backed tests.

| Command | Result |
|---|---|
| `git diff --check` | Passed; LF/CRLF warnings only. |
| `cargo fmt --all --check` | Passed. |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed. |
| `cargo sqlx migrate run` | Passed from an empty database through Phase 0, 1A, 1A RLS fix, and 1B RLS API context migrations. |
| `cargo sqlx prepare --check --workspace` | Passed with `DATABASE_URL` pointing at the temporary local Postgres. |
| `cargo test --workspace` | Passed with `DATABASE_URL` pointing at the temporary local Postgres. |
| `pnpm lint` | Passed. |
| `pnpm typecheck` | Passed. |
| `pnpm test` | Passed. |
| `pnpm test:e2e` | Passed. |

## Residual Risks

- OpenAPI is static JSON; route-contract tests reduce drift, but generated types remain future work.
- Idempotency is claimed before some state writes in the service layer; move claim and write into one repository transaction when the repository boundary exposes it.
- E2E uses a real Rust API process, but it is still API-client-level Vitest rather than browser automation.
- Real email and real OIDC providers remain intentionally unimplemented for Phase 1.
- Compose validation requires required `.env` secret values; bare `docker compose config` fails without them by design.
