# Detailed Stage Acceptance Fixture — v2.21

```json
{
  "stage": "S10",
  "purpose": "验证迁移、备份、恢复、projection rebuild、RAG rebuild 和 rollback runbook 可执行。",
  "inputs": {
    "database": "trpg_test",
    "backup_object": "backups/campaign_001.snapshot",
    "migration": "20260702_add_event_indexes",
    "projection": "campaign_public_timeline"
  },
  "actions": [
    {
      "id": "migration_check",
      "type": "sqlx_migrate"
    },
    {
      "id": "backup",
      "type": "backup_job"
    },
    {
      "id": "restore",
      "type": "restore_job"
    },
    {
      "id": "projection_rebuild",
      "type": "projection_rebuild"
    },
    {
      "id": "rollback_dry_run",
      "type": "runbook_rollback"
    }
  ],
  "expected_events": [
    {
      "type": "MigrationApplied"
    },
    {
      "type": "BackupCompleted",
      "hash": "sha256:backup-s10-deterministic"
    },
    {
      "type": "RestoreVerified"
    },
    {
      "type": "ProjectionRebuildVerified"
    }
  ],
  "expected_records": [
    {
      "record": "BackupManifest",
      "required_fields": [
        "object_key",
        "sha256",
        "created_at",
        "schema_version"
      ]
    },
    {
      "record": "RunbookExecutionRecord",
      "required_fields": [
        "operator",
        "command",
        "exit_code",
        "evidence_path"
      ]
    }
  ],
  "expected_errors": [
    {
      "case": "restore_hash_mismatch",
      "error": "RESTORE_HASH_MISMATCH"
    },
    {
      "case": "irreversible_migration_without_runbook",
      "error": "ROLLBACK_RUNBOOK_REQUIRED"
    }
  ],
  "failure_cases": [
    {
      "id": "projection_rebuild_differs",
      "expected_error": "PROJECTION_REBUILD_HASH_MISMATCH"
    }
  ],
  "required_evidence": [
    "evidence/stages/S10/migration-check.txt",
    "evidence/stages/S10/backup-restore.txt",
    "evidence/stages/S10/projection-rebuild.txt",
    "evidence/stages/S10/rollback-dry-run.md"
  ],
  "automation_target": "cargo sqlx migrate run && pwsh ./scripts/ops/backup-restore-smoke.ps1",
  "pass_criteria": [
    "migrations_idempotent",
    "backup_restore_hash_verified",
    "projection_rebuild_deterministic",
    "rollback_runbook_exists"
  ]
}
```
