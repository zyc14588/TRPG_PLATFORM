# Codex Batch 06 — Hardening, Full Gate, and Docs

Start only after Batch 05 is green.

## Read first

- `docs/p2/07_ACCEPTANCE_TEST_MATRIX.md`
- `docs/p2/08_STATUS_REPORT_TEMPLATE.md`
- `prompts/codex/P2_CHECK_COMMANDS.md`

## Tasks

1. Fill every acceptance matrix row with a test mapping or explicit deferred rationale.
2. Run full Rust, SQLx, and frontend gates.
3. Add negative tests for any missed security boundary.
4. Verify source package hygiene and no generated artifacts.
5. Update README reading order if needed.
6. Copy `docs/p2/08_STATUS_REPORT_TEMPLATE.md` to `docs/status/P2_STATUS.md` and fill it out.
7. Ensure no stale P1/P1.5 wording claims P2 is complete unless all gates pass.

## Checks

Run the full command list in `prompts/codex/P2_CHECK_COMMANDS.md`.

## Completion response

Include exact command output summary and P2 completion/deferred status.
