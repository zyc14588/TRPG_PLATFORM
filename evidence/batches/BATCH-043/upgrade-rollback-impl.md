# Upgrade Rollback Implementation Evidence

Prompt ID: `CODEX-0929-11-OPS-MIGRATION-02f99d0dd9`
Module: `upgrade_rollback_impl`
Event type: `OpsUpgradeRollbackImplRecorded`
Evidence path: `evidence/batches/BATCH-043/upgrade-rollback-impl.md`

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

- `migration_ledger`
- `rollback_plan`
- `event_store_hash`
- `projection_replay`

## Tests

- `cargo test -p trpg-ops --test upgrade_rollback_impl_contract_tests`
