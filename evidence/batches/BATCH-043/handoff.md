# BATCH-043 Handoff

## Completed

- Added `upgrade_rollback_impl` and `upgrade_rollback` ops runbook modules.
- Expanded `upgrade_rollback_impl` and `upgrade_rollback` beyond macro placeholders with policy gates, transaction evidence, external contract constants, and observability/audit records.
- Added contract and negative tests for both modules.
- Added BATCH-043 contract registry coverage.
- Added traceability records for all documentation-or-traceability prompts.
- Added supplemental requirement notes for all supplemental prompts.
- Added batch evidence and acceptance records.
- Added and ran S10 smoke scripts:
  - `scripts/backup_restore/smoke.sh`
  - `scripts/projection_rebuild/verify.sh`

## Next Batch Notes

- Do not re-run BATCH-043 unless metadata cleanup explicitly targets the primary-count mismatch.
- Future ops migration batches can use `all_batch_043_contracts()` as the registry for these two primary contracts.
- Keep any additional rollback behavior behind command/workflow/decision/event-store boundaries.
- S10 script checks now exist and pass via Git Bash on Windows.

## Open Items

- Upstream metadata says primary count `0`; normalized maps say primary count `2`.
- Docker CLI is unavailable locally; Docker Compose remains S09/S13 evidence, not a BATCH-043 script gate.
