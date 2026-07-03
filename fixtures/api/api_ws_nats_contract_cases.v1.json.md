# API WS NATS Contract Cases v1

```json
{
  "rest": [
    {
      "method": "POST",
      "path": "/api/v1/campaigns",
      "headers": {
        "Idempotency-Key": "idem_001"
      },
      "body": {
        "authority_mode": "AI_KP",
        "ruleset_id": "coc7"
      },
      "expected_status": 201
    },
    {
      "method": "PATCH",
      "path": "/api/v1/campaigns/camp_ai_harbor/authority",
      "body": {
        "authority_mode": "HUMAN_KP"
      },
      "expected_status": 409,
      "expected_error": "AuthorityContractImmutable"
    }
  ],
  "websocket": [
    {
      "type": "join_room",
      "principal": "user_player_a",
      "campaign_id": "camp_ai_harbor",
      "expected": "joined"
    },
    {
      "type": "scene_delta",
      "visibility": "private_to_player:user_player_a",
      "must_deliver_to": "user_player_a",
      "must_not_deliver_to": "user_player_b"
    }
  ],
  "nats_subjects": {
    "allowed": [
      "trpg.game.event.appended",
      "trpg.projection.updated",
      "trpg.agent.run.completed",
      "trpg.audit.recorded"
    ],
    "denied_patterns": [
      "trpg.llm.direct.*"
    ]
  }
}
```
