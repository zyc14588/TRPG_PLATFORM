# Test Data — Fork Lineage Cases

```json
[
  {
    "source_campaign_id":"camp_ai_harbor",
    "fork_source_session_id":"session_002",
    "new_campaign_id":"camp_human_harbor_whatif",
    "canon_status":"what-if",
    "fork_reason":"player_requested_human_kp_branch",
    "copy_default":["character_state","public_events","discovered_clues","world_state","npc_state","scene_state"],
    "copy_requires_explicit_permission":["keeper_notes","hidden_clues","private_messages","ai_internal_memory"],
    "expected_parent_unchanged":true
  }
]
```
