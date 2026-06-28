# Codex Batch 07 — Security, Legal, Privacy, and Final Gate

Start only after Batch 06 is green.

## Read first

- `CODEX_P2_MASTER_PROMPT.md`
- `docs/p2/INDEX.md`
- `docs/p2/00_EXECUTION_RULES.md`
- `docs/p2/02_BATCH_PLAN.md`
- `docs/p2/09_SECURITY_LEGAL_PRIVACY_PROVIDER_POLICY.md`
- `docs/p2/10_ACCEPTANCE_MATRIX.md`
- `docs/p2/12_STATUS_REPORT_TEMPLATE.md`
- `prompts/codex/P2_CHECK_COMMANDS.md`

## Tasks

1. Fill every acceptance matrix row with a test mapping or explicit deferred rationale.
2. Run the full Rust, SQLx, and frontend gates from `prompts/codex/P2_CHECK_COMMANDS.md`.
3. Add negative tests for any missed security, legal, privacy, provider, or visibility boundary.
4. Verify source package hygiene and no tracked generated artifacts.
5. Copy `docs/p2/12_STATUS_REPORT_TEMPLATE.md` to `docs/status/P2_STATUS.md` if missing, then fill it out honestly.
6. Ensure no stale P1/P1.5 wording claims P2 is complete unless all gates pass.

## Constraints

- Only hardening, docs, test mapping, and small bug fixes are in scope.
- Do not expand P2 product scope.
- Do not weaken acceptance rows or security tests to pass the gate.

## Checks

Run the full command list in `prompts/codex/P2_CHECK_COMMANDS.md`.

## Completion response

Include exact command output summary and P2 completion/deferred status.
