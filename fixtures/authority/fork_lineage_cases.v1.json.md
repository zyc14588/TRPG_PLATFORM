# Fork Lineage Cases v1

```json
{
  "forks": [
    {
      "source_campaign_id": "camp_ai_harbor",
      "fork_source_session_id": "session_002",
      "new_campaign_id": "camp_human_harbor_whatif",
      "canon_status": "what-if",
      "fork_reason": "player_requested_human_kp_branch",
      "snapshot_hash": "sha256:9e5d1b0c5d0838e2a81b72b3d3f361ba58ee03b06f6df5a0e93e1b93cf90b5ae",
      "copy_default": [
        "character_state",
        "public_events",
        "discovered_clues",
        "world_state",
        "npc_state",
        "scene_state"
      ],
      "copy_requires_explicit_permission": [
        "keeper_notes",
        "hidden_clues",
        "private_messages",
        "ai_internal_memory"
      ],
      "expected": {
        "parent_unchanged": true,
        "child_contract_locked": true,
        "parent_authority_mode": "AI_KP",
        "child_authority_mode": "HUMAN_KP"
      }
    }
  ]
}
```
