# Codex Batch 06 — Frontend UI/UX

Compatibility note: this file keeps its legacy name for existing links. It is the B06 Frontend UI/UX prompt. Hardening and final acceptance moved to B07.

Start only after Batch 05 is green.

## Read first

- `CODEX_P2_MASTER_PROMPT.md`
- `docs/p2/INDEX.md`
- `docs/p2/00_EXECUTION_RULES.md`
- `docs/p2/02_BATCH_PLAN.md`
- `docs/p2/08_FRONTEND_UI_UX.md`
- `schemas/openapi.json`
- existing frontend backend/client tests

## Tasks

1. Add typed frontend client functions for P2 endpoints.
2. Add minimal pages/components for document ingest/paste, pending review, and RAG evidence display.
3. Show citation, provenance, license, and visibility-safe fields in query results.
4. Ensure PL/observer UI never renders KP review controls or hidden fields.
5. Add fake-backend tests for DTO privacy and evidence rendering.
6. Keep UI simple; no generated final-answer UX in P2.

## Constraints

- Do not trust UI to enforce security; API must deny.
- Hidden fields must be absent from DTO tests, not merely hidden in CSS.
- Do not introduce unpinned production dependencies without reason.

## Checks

```powershell
pnpm install --frozen-lockfile
pnpm lint
pnpm typecheck
pnpm test
pnpm test:e2e
pnpm build
```

## Completion response

List routes/components, client functions, tests, and any UX limitations.
