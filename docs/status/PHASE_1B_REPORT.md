# Phase 1B Report

Status: verification complete; waiting for human review.

Scope verified:

- Auth REST routes: Magic Link request/verify, OIDC mock start/callback, refresh rotation, logout revoke, CSRF, and `/api/me`.
- Room REST routes: create/list/get rooms, invite creation, invite accept, member listing, idempotent create replay, and non-member isolation.
- OpenAPI: static contract at `schemas/openapi.json`, served by `GET /openapi.json`, with route-contract tests against registered Axum routes.
- Frontend/backend E2E flow: login, room creation, invite/join flow, and third-party rejection through the frontend API client against a real Rust API process. `FakeBackend` remains only for narrow client unit tests.

Requested test coverage:

| # | Check | Coverage |
|---:|---|---|
| 1 | Magic Link single use | `magic_link_is_single_use` |
| 2 | Magic Link expiry | `magic_link_expires` |
| 3 | Refresh rotation | `refresh_rotates_refresh_cookie_and_rejects_reuse` |
| 4 | Logout revoke | `logout_revokes_current_refresh_session` |
| 5 | CSRF protection | `csrf_is_required_for_refresh` |
| 6 | Unauthenticated access | `me_rejects_unauthenticated_access`, `room_creation_rejects_unauthenticated_access` |
| 7 | Room creation | `room_creation_list_and_safe_projection_work` |
| 8 | Duplicate idempotency key | `duplicate_room_create_idempotency_key_returns_original_room`, frontend duplicate replay test |
| 9 | Owner invites member | `owner_invites_member_and_invited_user_joins` |
| 10 | Non-owner cannot invite | `non_owner_member_cannot_invite` |
| 11 | Invited user joins | `owner_invites_member_and_invited_user_joins` |
| 12 | Third party cannot access | `third_party_and_cross_room_access_do_not_leak_private_rooms`, frontend E2E |
| 13 | Cross-room data isolation | `third_party_and_cross_room_access_do_not_leak_private_rooms`, `repository_get_room_member_is_room_scoped`, `postgres_http_room_flow_uses_rls_context_and_writes_audit` |
| 14 | OpenAPI matches routes | `openapi_contract_is_readable_and_matches_registered_routes` |
| 15 | Frontend login flow | `runs the frontend login flow without storing refresh tokens` |
| 16 | Room creation flow | `runs room creation and duplicate idempotency-key replay` |
| 17 | Two-user invite/join E2E | `frontend auth and room E2E flow` |
| 18 | Network error and duplicate submit | `reports network errors clearly`, duplicate replay tests |
| 19 | Audit rows for login/create/invite/accept/denied access | `http_vertical_flow_writes_required_audit_rows`, `postgres_http_room_flow_uses_rls_context_and_writes_audit` |

OpenAPI change summary:

- No new OpenAPI path was added during this verification pass.
- `schemas/openapi.json` documents system, auth, and room routes.
- Security schemes cover bearer access tokens, `trpg_refresh` HttpOnly cookie, and `x-csrf-token` double-submit CSRF header.
- `RoomDto` exposes only `id`, `title`, `system_name`, `privacy_mode`, `version`, and `my_role`; no KP-only field is in the contract.

Frontend/backend contract summary:

- Frontend stores only access token, access expiry, CSRF token, and user summary in `sessionStorage`.
- Refresh token stays in the `trpg_refresh` HttpOnly cookie.
- Refresh/logout send `x-csrf-token`; backend compares it with the `trpg_csrf` cookie.
- Development Magic Link may return `development_magic_link`; production returns `null`.
- Missing `TRPG_AUTH_MODE` now defaults to production; development auth must be explicitly enabled.
- Room create request uses `title`, `system_name`, `privacy_mode`, and `idempotency_key`.
- Invitation create request uses `email`, `role`, and `idempotency_key`; `owner` is not accepted as an invitation role.
- Invitation accept request uses path-bound `token` plus `idempotency_key`.
- Runtime DTO validation rejects malformed room DTOs and explicitly rejects KP-only room fields.
- Next.js can proxy same-origin `/api/*` and `/openapi.json` to the Rust API with `TRPG_API_PROXY_TARGET`.

Verification results:

| Command | Result |
|---|---|
| `git diff --check` | Passed; Git emitted LF/CRLF warnings only. |
| `cargo fmt --all --check` | Passed. |
| `cargo check --workspace --all-targets` | Passed. |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed. |
| `cargo test --workspace` | Passed without `DATABASE_URL`; database-dependent tests can no-op in that mode. |
| `cargo sqlx migrate run` | Passed against temporary `pgvector/pgvector:pg17` on `127.0.0.1:55432`; includes `20260626012000_phase_1b_rls_api_context.sql`. |
| `cargo sqlx prepare --check --workspace` | Initial bare run failed because `DATABASE_URL` was unset; passed with the temporary database. |
| `cargo test --workspace` with `DATABASE_URL` | Passed; database-backed storage/RLS/migration tests executed. |
| `pnpm lint` | Passed. |
| `pnpm typecheck` | Passed. |
| `pnpm test` | Passed; 2 files, 6 tests. |
| `pnpm test:e2e` | Passed; 1 file, 1 test. |
| `pnpm build` | Passed; Next generated 8 static pages. |
| `docker compose config` | Bare run failed because required `.env` value `GRAFANA_ADMIN_PASSWORD` was missing; passed with temporary placeholder environment variables. |

Environment notes:

- Temporary SQLx container `trpg-sqlx-p1-fix` was removed after verification.
- Compose Postgres was started for inspection and then stopped again.
- `pnpm typecheck` / `pnpm build` updated the tracked `apps/web/tsconfig.tsbuildinfo` generated file.

Known risks:

- OpenAPI is still static JSON; route-contract tests reduce drift, but generated client/types remain future work.
- Idempotency is claimed before some state writes in the service layer; move claim and write into one repository transaction when the repository boundary exposes it.
- E2E now talks to a real Rust API process, but it is still API-client-level Vitest, not browser automation.
- Real email and real OIDC providers remain intentionally unimplemented for P1B.
- Compose validation requires `.env` values for required secrets; bare `docker compose config` fails by design without them.

Stopped at P1B. No RAG, Agent, WebSocket, maps, audio, clue graph, or Creator image work was started.
