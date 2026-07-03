# Test Data — Permission Matrix Cases

```json
[
  {"actor_role":"ServerOwner","action":"pause_room","expected":"ALLOW"},
  {"actor_role":"ServerOwner","action":"override_dice_roll","expected":"DENY"},
  {"actor_role":"Moderator","action":"mute_player","expected":"ALLOW"},
  {"actor_role":"Moderator","action":"change_game_decision","expected":"DENY"},
  {"actor_role":"HumanKP","authority_mode":"HUMAN_KP","action":"confirm_agent_draft","expected":"ALLOW"},
  {"actor_role":"Player","authority_mode":"AI_KP","action":"request_reconsideration","expected":"ALLOW"},
  {"actor_role":"Player","authority_mode":"AI_KP","action":"override_ai_decision","expected":"DENY"}
]
```
