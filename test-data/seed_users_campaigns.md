# Test Data — Seed Users / Campaigns

```json
{
  "users": [
    {"id":"user_owner","role":"ServerOwner","display_name":"Server Owner"},
    {"id":"user_campaign_owner","role":"CampaignOwner","display_name":"Campaign Owner"},
    {"id":"user_human_kp","role":"HumanKP","display_name":"Keeper Lin"},
    {"id":"user_player_a","role":"Player","display_name":"Investigator A"},
    {"id":"user_player_b","role":"Player","display_name":"Investigator B"},
    {"id":"user_spectator","role":"Spectator","display_name":"Spectator"},
    {"id":"user_moderator","role":"Moderator","display_name":"Moderator"}
  ],
  "ai_kp_profiles": [
    {"id":"ai_kp_local_level4","provider":"ollama","model_id":"local-codex-kp-test","certification_level":"LOCAL_MODEL_LEVEL_4"},
    {"id":"ai_kp_uncertified","provider":"llama_cpp","model_id":"uncertified-json-unstable","certification_level":"LOCAL_MODEL_LEVEL_2"}
  ],
  "campaigns": [
    {"id":"camp_human_archive","authority_mode":"HUMAN_KP","authority_owner":"user_human_kp","ruleset_id":"coc7"},
    {"id":"camp_ai_harbor","authority_mode":"AI_KP","authority_owner":"ai_kp_local_level4","ruleset_id":"coc7"}
  ]
}
```
