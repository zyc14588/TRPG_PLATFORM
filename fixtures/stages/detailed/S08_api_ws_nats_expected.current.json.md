# Detailed Stage Acceptance Fixture — v2.21

```json
{
  "stage": "S08",
  "purpose": "验证 REST/OpenAPI、WebSocket 房间同步、NATS subject contract 和幂等请求。",
  "inputs": {
    "api_call": {
      "method": "POST",
      "path": "/campaigns/{id}/actions",
      "idempotency_key": "idem_001"
    },
    "websocket_message": {
      "type": "player_action",
      "correlation_id": "corr_001"
    },
    "nats_subject": "campaign.campaign_001.event.created"
  },
  "actions": [
    {
      "id": "openapi_contract",
      "type": "api_contract_test"
    },
    {
      "id": "ws_reconnect",
      "type": "websocket_contract_test"
    },
    {
      "id": "nats_publish",
      "type": "nats_subject_contract_test"
    }
  ],
  "expected_events": [
    {
      "type": "ApiRequestAccepted",
      "idempotency_key": "idem_001"
    },
    {
      "type": "WebSocketStateSynced",
      "room": "campaign_001"
    },
    {
      "type": "NatsMessagePublished",
      "subject": "campaign.campaign_001.event.created"
    }
  ],
  "expected_records": [
    {
      "record": "ApiAuditRecord",
      "required_fields": [
        "actor",
        "correlation_id",
        "idempotency_key",
        "status"
      ]
    },
    {
      "record": "RealtimeDeliveryRecord",
      "required_fields": [
        "session_id",
        "visible_to",
        "sequence"
      ]
    }
  ],
  "expected_errors": [
    {
      "case": "missing_idempotency_key",
      "error": "IDEMPOTENCY_KEY_REQUIRED"
    },
    {
      "case": "ws_private_leak",
      "error": "REALTIME_VISIBILITY_VIOLATION"
    },
    {
      "case": "invalid_nats_subject",
      "error": "NATS_SUBJECT_CONTRACT_VIOLATION"
    }
  ],
  "failure_cases": [
    {
      "id": "duplicate_action_not_idempotent",
      "expected_error": "IDEMPOTENCY_CONTRACT_BROKEN"
    }
  ],
  "required_evidence": [
    "evidence/stages/S08/openapi-contract.txt",
    "evidence/stages/S08/websocket-contract.txt",
    "evidence/stages/S08/nats-contract.txt"
  ],
  "automation_target": "cargo test -p trpg-api --test s08_fixture_acceptance_contract_tests --all-features",
  "pass_criteria": [
    "openapi_stable",
    "websocket_visibility_enforced",
    "nats_subjects_versioned",
    "idempotency_required"
  ]
}
```
