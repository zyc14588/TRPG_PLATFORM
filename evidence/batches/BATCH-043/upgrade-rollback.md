# Upgrade Rollback Evidence

Prompt ID: `CODEX-0945-11-OPS-MIGRATION-fab61f7e5e`
Module: `upgrade_rollback`
Event type: `OpsUpgradeRollbackRecorded`
Evidence path: `evidence/batches/BATCH-043/upgrade-rollback.md`

## Governance

- Formal writes use `CommandEnvelope` and `AuthorityContract`.
- Events are appended through the shared ops event store helper.
- Direct agent writes are rejected by the shared contract tests.
- Event replay and visibility boundaries remain owned by the event store and visibility model.
- Tool Permission, OpenFGA, and OPA gates fail closed before Event Store append.
- Transaction evidence records `sqlx_event_store_transaction_boundary`, `event_store_append_only`, expected version, and appended event sequence.
- OpenAPI operation, event schema, NATS subject, OpenFGA relation, OPA policy, tracing span, metric, and audit action are current-safe constants.
- Observability evidence carries `correlation_id` and `causation_id`.

## Read Models

- `backup_manifest`
- `rollback_plan`
- `event_store_hash`
- `restore_verification`

## Tests

- `cargo test -p trpg-ops --test upgrade_rollback_contract_tests`
