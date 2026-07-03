# Test Data — Event Store Stream Cases

```json
{
  "stream_id":"campaign-camp_ai_harbor",
  "events":[
    {"version":1,"type":"CampaignCreated","authority_mode":"AI_KP","visibility":"system_only"},
    {"version":2,"type":"AuthorityContractLocked","locked":true,"visibility":"system_only"},
    {"version":3,"type":"CharacterSheetSubmitted","character_id":"char_a","visibility":"private_to_player:user_player_a"},
    {"version":4,"type":"DiceRolled","roll_id":"roll_001","formula":"1d100","final_result":37,"visibility":"party_visible"},
    {"version":5,"type":"ClueRevealed","clue_id":"clue_wrong_signature","visibility":"party_visible"}
  ],
  "append_tests":[
    {"expected_version":5,"append":"SessionSummaryCreated","expected":"ALLOW"},
    {"expected_version":4,"append":"SessionSummaryCreated","expected":"VERSION_CONFLICT"},
    {"idempotency_key":"idem_repeat","append_twice":true,"expected":"SECOND_RETURNS_EXISTING"}
  ]
}
```
