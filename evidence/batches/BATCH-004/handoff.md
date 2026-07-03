# BATCH-004 Handoff

## Completed

- Added current-safe B004 shared-kernel modules:
  `constitution`, `document_set`, `system_context`, `readme`,
  `workspace_and_governance`, `cargo_workspace_impl`, `constitution_impl`, and
  `document_set_impl`.
- Added contract tests for all 8 B004 primary rows.
- Registered the B004 modules in `crates/trpg-shared-kernel/src/lib.rs`.
- Recorded B004 plan, traceability, changed files, test output, and acceptance
  report under `evidence/batches/BATCH-004/`.
- Ran minimal shared-kernel tests, S01 cargo checks, static boundary scans, and
  S01 fixture-declared assertions.

## Next Batch Notes

- Do not start B005 or later work unless explicitly instructed.
- Continue applying the normalized execution map, current-safe module/output
  map, and token rewrite table before reading per-file prompts.
- Keep historical source names, old version tokens, source hashes, and archived
  paths as provenance only.
- Treat B004 supplemental prompts as already traced; do not create separate
  Rust outputs for them unless a later primary prompt grants ownership.
- Later batches may build runtime/API/eventing surfaces, but this batch did not
  add migrations, NATS subjects, provider adapters, or workflow handlers.

## Open Risks

- None blocking for B004.
- Full S01 acceptance still depends on the remaining scheduled S01 batches.
