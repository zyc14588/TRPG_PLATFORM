# S01 Acceptance Evidence

Stage: `S01`
Current batch: `BATCH-003-01-foundation`
Conclusion: PASS.

## Evidence Paths

- `evidence/batches/BATCH-003/plan.md`
- `evidence/batches/BATCH-003/prompt-traceability.md`
- `evidence/batches/BATCH-003/changed-files.txt`
- `evidence/batches/BATCH-003/test-output.txt`
- `evidence/batches/BATCH-003/acceptance-report.md`
- `evidence/stages/S01/kernel-contract-tests.txt`

## Acceptance Coverage

- Typed IDs reject empty IDs.
- Visibility labels are closed and reject unknown labels.
- Stable error codes are exposed through `TrpgError::code`.
- Command envelopes include idempotency key, expected version, actor,
  authority mode, authority contract version, visibility, fact provenance,
  correlation id, causation id, and write path.
- Event append enforces expected version and idempotency.
- Visibility replay redacts restricted events.
- Authority Contract mode/version cannot be mutated in place.
- Shared kernel dependency direction rejects domain/runtime/api/agent
  dependencies from the shared kernel layer.

