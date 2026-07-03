# AI Decision Record Cases v1

```json
{
  "decisions": [
    {
      "decision_id": "dec_spot_hidden_desk",
      "authority_mode": "AI_KP",
      "agent_id": "coc_ai_keeper_orchestrator",
      "player_action": "检查书桌底部是否有暗格",
      "public_decision_summary": {
        "must_contain": [
          "需要侦查检定",
          "难度",
          "骰子结果"
        ],
        "must_not_contain": [
          "keeper_truth",
          "secret_operator"
        ]
      },
      "keeper_decision_notes": {
        "visibility": "keeper_only",
        "may_contain": [
          "clue_012_nearby"
        ]
      },
      "audit_decision_record": {
        "must_contain": [
          "agent_pack_version",
          "prompt_version",
          "model_provider",
          "model_id",
          "context_hash",
          "tool_calls",
          "linked_events"
        ]
      },
      "expected_events": [
        "DecisionProposed",
        "DiceRolled",
        "DecisionCommitted"
      ]
    }
  ]
}
```
