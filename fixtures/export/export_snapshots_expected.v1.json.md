# Export Snapshots Expected v1

```json
{
  "player_export": {
    "must_contain": [
      "public_scene_summary",
      "discovered_clues",
      "visible_dice_rolls"
    ],
    "must_not_contain": [
      "keeper_truth",
      "npc_true_identity",
      "ai_internal",
      "private_to_other_player"
    ]
  },
  "keeper_export": {
    "must_contain": [
      "keeper_truth",
      "hidden_clues",
      "npc_secrets",
      "all_public_events"
    ],
    "must_not_contain": [
      "ai_internal_raw_prompt_chain"
    ]
  },
  "audit_export": {
    "must_contain": [
      "dice_rolls",
      "decision_records",
      "model_route_snapshot",
      "tool_calls",
      "visibility_labels"
    ],
    "must_redact_for_non_admin": [
      "api_keys",
      "private_messages_without_permission"
    ]
  }
}
```
