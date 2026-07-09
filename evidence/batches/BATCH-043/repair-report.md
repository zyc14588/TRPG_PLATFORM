# BATCH-043 Repair Report

## Scope

Primary-only repair for:

- `CODEX-0929-11-OPS-MIGRATION-02f99d0dd9`
- `CODEX-0945-11-OPS-MIGRATION-fab61f7e5e`

No supplemental or documentation-or-traceability prompt scope was expanded.

## Fixed Findings

- Replaced primary macro-only placeholders with concrete module contracts and executable service paths.
- Added Tool Permission, OpenFGA, and OPA policy gates that fail closed with `PolicyDenied`.
- Added SQLx/EventStore transaction evidence structs tied to expected version and appended event sequence.
- Added OpenAPI operation, event schema, NATS subject, OpenFGA relation, OPA policy, tracing span, metric, and audit constants with current-safe names.
- Added observability records carrying `correlation_id` and `causation_id`.
- Added direct tests for `keeper_only`, `private_to_player`, and `ai_internal` redaction/replay boundaries.
- Added and ran S10 script checks:
  - `scripts/backup_restore/smoke.sh`
  - `scripts/projection_rebuild/verify.sh`

## Verification

All commands listed in `evidence/batches/BATCH-043/test-output.txt` passed after repair.
