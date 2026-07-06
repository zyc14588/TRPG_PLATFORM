# Detailed Stage Acceptance Fixture — v2.21

```json
{
  "stage": "S03",
  "purpose": "验证 Event Store append-only、expected_version 并发控制、projection replay hash 和 outbox/idempotency。",
  "inputs": {
    "stream_id": "campaign_001",
    "expected_version": 2,
    "events": [
      {
        "event_id": "evt_001",
        "type": "CampaignCreated"
      },
      {
        "event_id": "evt_002",
        "type": "AuthorityContractLocked"
      },
      {
        "event_id": "evt_003",
        "type": "SceneStarted"
      }
    ]
  },
  "actions": [
    {
      "id": "append_events",
      "type": "event_store_append"
    },
    {
      "id": "replay_projection",
      "type": "projection_rebuild"
    },
    {
      "id": "duplicate_command",
      "type": "idempotency_replay"
    }
  ],
  "expected_events": [
    {
      "type": "EventsAppended",
      "count": 3
    },
    {
      "type": "ProjectionRebuilt",
      "hash": "sha256:a83861bce178f274e6a2e809c790770577445268b48fedfb889af4b87f8c1c50"
    }
  ],
  "expected_records": [
    {
      "record": "OutboxMessage",
      "required_fields": [
        "correlation_id",
        "causation_id",
        "event_id"
      ]
    },
    {
      "record": "ProjectionCheckpoint",
      "required_fields": [
        "stream_id",
        "version",
        "projection_hash"
      ]
    }
  ],
  "expected_errors": [
    {
      "case": "wrong_expected_version",
      "error": "EVENT_STREAM_VERSION_CONFLICT"
    },
    {
      "case": "duplicate_idempotency_key",
      "error": "IDEMPOTENCY_REPLAYED"
    }
  ],
  "failure_cases": [
    {
      "id": "mutable_event_update",
      "expected_error": "EVENT_STORE_APPEND_ONLY"
    }
  ],
  "required_evidence": [
    "evidence/stages/S03/event-store-contract.txt",
    "evidence/stages/S03/projection-replay-hash.txt"
  ],
  "automation_target": "cargo test -p trpg-storage event_store projection_replay outbox --all-features",
  "pass_criteria": [
    "append_only",
    "version_conflict_detected",
    "projection_hash_stable",
    "outbox_written"
  ]
}
```
