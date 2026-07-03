# Test Data — Authority Contract Cases

```json
[
  {
    "case_id":"authority_locked_human_kp",
    "input":{"campaign_id":"camp_human_archive","authority_mode":"HUMAN_KP","authority_owner":"user_human_kp","locked":true},
    "attempt":{"actor":"user_campaign_owner","op":"change_authority_mode","value":"AI_KP"},
    "expected":{"result":"DENY","error":"AuthorityContractImmutable","events_appended":0}
  },
  {
    "case_id":"authority_locked_ai_kp_no_human_override",
    "input":{"campaign_id":"camp_ai_harbor","authority_mode":"AI_KP","authority_owner":"ai_kp_local_level4","locked":true},
    "attempt":{"actor":"user_campaign_owner","op":"override_ai_decision","decision_id":"dec_001"},
    "expected":{"result":"DENY","error":"AuthorityViolation","events_appended":0}
  },
  {
    "case_id":"fork_changes_authority_only_in_child",
    "input":{"source_campaign_id":"camp_ai_harbor","source_session_id":"session_001","new_authority_mode":"HUMAN_KP","new_authority_owner":"user_human_kp"},
    "expected":{"result":"ALLOW","parent_unchanged":true,"child_has_new_authority_contract":true,"canon_status":"what-if"}
  }
]
```
