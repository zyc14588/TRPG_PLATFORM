# Codex Batch 00 — P1.5 Fix Gate

You are in TRPG_PLATFORM. Complete only the P1.5 pre-P2 fix gate. Do not implement P2 RAG features.

## Read first

- `CODEX_P2_MASTER_PROMPT.md`
- `docs/P1_5_FIX_PLAN.md`
- `docs/SECURITY_RLS_POLICY.md`
- `docs/p2/00_P1_5_FIX_GATE.md`
- `prompts/codex/P2_CHECK_COMMANDS.md`

## Tasks

1. Verify `.env.example` supports local development without accidentally using production defaults.
2. Verify production boot rejects missing DB, short/default auth secret, and superuser `postgres` app role.
3. Resolve or document license declaration mismatch. If maintainer decision is needed, stop after creating a clear issue/status note; do not guess.
4. Confirm `/healthz`, `/readyz`, and `/metrics` implementation. If absent, implement minimal routes, tests, and OpenAPI entries.
5. Remove tracked generated artifacts such as `tsconfig.tsbuildinfo`.
6. Replace stale frontend phase copy.
7. Pin or rationalize frontend dependency ranges; do not do broad upgrades.
8. Document and test CSRF/auth mutation policy.
9. Do not add RAG APIs in this batch.

## Acceptance

- P1.5 gate commands in `docs/p2/00_P1_5_FIX_GATE.md` pass.
- No generated artifact remains tracked.
- Status summary lists exact commands run.

## Completion response

Use:

```md
## Batch summary
- Batch: 00 Fix Gate
- Files changed:
- Tests run:
- Results:
- Acceptance criteria met:
- Deferred items:
```
