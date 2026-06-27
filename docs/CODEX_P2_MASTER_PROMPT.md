# CODEX P2 MASTER PROMPT — TRPG_PLATFORM

You are working in the TRPG_PLATFORM repository. Your mission is to implement Phase 2: Rules / RAG / Document Ingestion, after the P1.5 fix gate passes.

## Mandatory reading order

Read these files before modifying code:

1. `README.md`
2. `AGENTS.md` if present
3. `CODEX_MASTER_PROMPT.md` if present
4. `docs/P1_5_FIX_PLAN.md`
5. `docs/SECURITY_RLS_POLICY.md`
6. `docs/LEGAL_POLICY.md`
7. `docs/RAG_DESIGN.md`
8. `docs/P2_CODEX_HANDOFF.md`
9. `docs/P2_RAG_IMPLEMENTATION_SPEC.md`
10. `docs/P2_RAG_ACCEPTANCE_TESTS.md`
11. `docs/p2/INDEX.md`
12. `docs/p2/00_P1_5_FIX_GATE.md`
13. The current batch prompt in `prompts/codex/`.

## Global constraints

- Do not add unauthorized commercial rule text.
- Do not call real cloud providers in tests.
- Do not bypass ABAC/RLS; DB policies must enforce license and visibility boundaries.
- Do not let `pending_review` or `denied` content enter ordinary chunking, embedding, indexing, or retrieval.
- Do not let KP-only/module/private visibility leak to PL/observer/public screen routes.
- Do not use the `postgres` superuser as a production application login role.
- Do not introduce unbounded `top_k`, unbounded upload size, or unbounded chunk size.
- Do not implement full WebSocket/Redis/outbox replay in P2 unless the P2 core is complete and explicitly requested.

## Batch discipline

Work in batches. A batch may not start until the previous batch gate passes. If a batch discovers a blocker, fix the blocker inside the same batch only when it is required for that batch; otherwise document it in the status report and stop.

Each batch completion message must include:

```md
## Batch summary
- Batch:
- Files changed:
- Tests run:
- Results:
- Acceptance criteria met:
- Deferred items:
```

## Definition of P2 done

P2 is complete only when all of the following are true:

- `cargo fmt --all --check` passes.
- `cargo check --workspace` passes.
- `cargo clippy --workspace --all-targets -- -D warnings` passes.
- `cargo test --workspace` passes.
- SQLx migrations run on a fresh database.
- `cargo sqlx prepare --check --workspace` passes.
- `pnpm install --frozen-lockfile`, `pnpm lint`, `pnpm typecheck`, `pnpm test`, `pnpm test:e2e`, and `pnpm build` pass.
- Retrieval returns evidence/citation/provenance, not final generated answers.
- Direct DB role tests prove normal retrieval cannot read pending/denied or invisible chunks.
- LocalOnly mode never invokes cloud model, embedding, rerank, OCR, or image providers.
- `docs/p2/08_STATUS_REPORT_TEMPLATE.md` is filled out and committed as `docs/status/P2_STATUS.md`.
