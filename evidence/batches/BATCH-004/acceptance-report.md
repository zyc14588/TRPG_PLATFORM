# BATCH-004 Acceptance Report

Stage: `S01`
Batch: `BATCH-004-01-foundation`
Conclusion: PASS for current batch scope.

## Scope Decision

The operator input stated that 0 primary prompts were recognized. The
authoritative `batches/B004.md` table lists 8 primary implementation rows and
17 supplemental rows. This batch followed the table after applying the
normalized current-safe maps.

## Acceptance Checks

- Required authority, bootstrap, top-level design, normalized maps, operation
  guides, S01 prompts, B004 batch file, and B004 per-file prompts were read.
- All 8 B004 primary rows have current-safe flat Rust source and contract test
  files.
- All 17 B004 supplemental rows were treated as supplemental only and did not
  create independent Rust outputs.
- `lib.rs` was updated only to expose the B004 modules.
- Command-envelope checks include idempotency key, expected version, actor,
  authority mode/version, visibility, fact provenance, correlation id, and
  causation id.
- Formal review writes go through `CommandEnvelope` plus `EventStore::append`.
- Direct model-provider access, direct agent state writes, Authority Contract
  mutation, and non-event-store canonical state boundaries are rejected.
- Visibility and fact provenance are preserved on governed review events.
- No migrations, API handlers, NATS consumers, provider adapters, workflows,
  or database writes were added.
- Rust src/tests contain no template module names, untyped `serde_json::Value`,
  direct provider strings, or historical version/path tokens checked by static
  scans.

## Evidence

- Work plan: `evidence/batches/BATCH-004/plan.md`
- Prompt traceability: `evidence/batches/BATCH-004/prompt-traceability.md`
- Changed files: `evidence/batches/BATCH-004/changed-files.txt`
- Test output: `evidence/batches/BATCH-004/test-output.txt`
- Handoff: `evidence/batches/BATCH-004/handoff.md`

## Risks And Exceptions

- The worktree contains many modified or untracked files from previous batches;
  this batch did not revert or normalize them.
- B004 remains an S01 shared-kernel governance contract layer. Runtime
  provider adapters, API schemas, migrations, NATS subjects, and workflow
  handlers remain outside this batch unless later primary prompts authorize
  them.
- A first S01 fixture assertion script used token names not present in the
  fixture and failed; the corrected fixture-declared assertion passed.

## Strict Conclusion

PASS. BATCH-004 implemented the allowed current-safe shared-kernel governance
contracts and tests without expanding scope or weakening top-level governance
red lines.
