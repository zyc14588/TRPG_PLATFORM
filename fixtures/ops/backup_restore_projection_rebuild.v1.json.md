# Backup Restore Projection Rebuild v1

```json
{
  "backup_restore": {
    "source_stream": "campaign-camp_ai_harbor",
    "before_backup_event_hash": "sha256:ea4d9a5f8aa58929b9514a881c876e66557ee9706e82a2e251cdb247c6f4141b",
    "restore_target": "clean_database",
    "expected_after_restore_event_hash": "sha256:ea4d9a5f8aa58929b9514a881c876e66557ee9706e82a2e251cdb247c6f4141b",
    "projection_rebuild": {
      "command": "rebuild_projection",
      "may_append_events": false,
      "expected_projection_hash": "sha256:15d1765619e2e0b512f21a2a1ccfb56b727efecbeadccbbd919ee88199e747e2"
    }
  },
  "health_checks": [
    {
      "service": "api",
      "endpoint": "/healthz",
      "expected": "healthy"
    },
    {
      "service": "realtime",
      "endpoint": "/healthz",
      "expected": "healthy"
    },
    {
      "service": "agent-worker",
      "endpoint": "/healthz",
      "expected": "healthy"
    }
  ]
}
```
