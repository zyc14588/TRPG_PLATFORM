# Codex Batch 05 — Frontend RAG UI

Start only after Batch 04 is green.

## Read first

- `docs/p2/05_FRONTEND_RAG_UI_SPEC.md`
- `docs/p2/04_SERVER_API_OPENAPI_SPEC.md`
- `schemas/openapi.json`
- existing frontend backend/client tests

## Tasks

1. Add typed frontend client functions for P2 endpoints.
2. Add minimal pages/components for:
   - document ingest/paste
   - pending review
   - RAG query evidence display
3. Show citation/provenance fields in query results.
4. Ensure PL/observer UI does not render KP review controls.
5. Add fake backend tests for DTO privacy and evidence rendering.
6. Keep UI simple; no generated final answer UX in P2.

## Constraints

- Do not trust UI to enforce security; API must deny.
- Hidden fields must be absent from DTO tests, not merely hidden in CSS.
- Do not introduce unpinned production dependencies without reason.

## Checks

```bash
pnpm install --frozen-lockfile
pnpm lint
pnpm typecheck
pnpm test
pnpm test:e2e
pnpm build
```

## Completion response

List routes/components, client functions, tests, and any UX limitations.
