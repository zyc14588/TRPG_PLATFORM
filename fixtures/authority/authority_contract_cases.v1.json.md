# Authority Contract Cases v1

```json
{
  "cases": [
    {
      "case_id": "authority_locked_human_kp",
      "given": {
        "campaign_id": "camp_human_archive",
        "authority_mode": "HUMAN_KP",
        "authority_owner": "user_human_kp",
        "locked": true,
        "contract_version": 1
      },
      "attempt": {
        "actor": "user_campaign_owner",
        "command": "ChangeAuthorityMode",
        "value": "AI_KP",
        "expected_version": 7,
        "idempotency_key": "idem-authority-001"
      },
      "expect": {
        "result": "DENY",
        "error": "AuthorityContractImmutable",
        "events_appended": 0,
        "audit_required": true
      }
    },
    {
      "case_id": "authority_locked_ai_kp_no_human_override",
      "given": {
        "campaign_id": "camp_ai_harbor",
        "authority_mode": "AI_KP",
        "authority_owner": "ai_kp_local_level4",
        "locked": true
      },
      "attempt": {
        "actor": "user_campaign_owner",
        "command": "OverrideAIDecision",
        "decision_id": "dec_001"
      },
      "expect": {
        "result": "DENY",
        "error": "AuthorityViolation",
        "events_appended": 0
      }
    },
    {
      "case_id": "fork_changes_authority_only_in_child",
      "given": {
        "source_campaign_id": "camp_ai_harbor",
        "source_session_id": "session_002",
        "source_authority_mode": "AI_KP"
      },
      "attempt": {
        "actor": "user_campaign_owner",
        "command": "ForkCampaign",
        "new_authority_mode": "HUMAN_KP",
        "new_authority_owner": "user_human_kp",
        "fork_reason": "player_requested_human_branch"
      },
      "expect": {
        "result": "ALLOW",
        "parent_unchanged": true,
        "child_has_new_authority_contract": true,
        "canon_status": "what-if",
        "copied_by_default": [
          "character_state",
          "public_events",
          "discovered_clues",
          "world_state"
        ],
        "requires_explicit_permission": [
          "keeper_notes",
          "hidden_clues",
          "private_messages",
          "ai_internal_memory"
        ]
      }
    }
  ]
}
```
