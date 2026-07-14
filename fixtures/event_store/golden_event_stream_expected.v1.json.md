# Golden Event Stream Expected v1

```json
{
  "stream_id": "campaign-camp_ai_harbor",
  "initial_version": 0,
  "events": [
    {
      "version": 1,
      "type": "CampaignCreated",
      "schema_version": 1,
      "authority_mode": "AI_KP",
      "visibility": "system_only"
    },
    {
      "version": 2,
      "type": "AuthorityContractLocked",
      "schema_version": 1,
      "locked": true,
      "visibility": "system_only"
    },
    {
      "version": 3,
      "type": "CharacterSheetSubmitted",
      "schema_version": 1,
      "character_id": "char_a",
      "visibility": "private_to_player:user_player_a"
    },
    {
      "version": 4,
      "type": "CharacterSheetVersionLocked",
      "schema_version": 1,
      "character_id": "char_a",
      "visibility": "private_to_player:user_player_a"
    },
    {
      "version": 5,
      "type": "DiceRolled",
      "schema_version": 1,
      "roll_id": "roll_001",
      "formula": "1d100",
      "final_result": 37,
      "visibility": "party_visible"
    },
    {
      "version": 6,
      "type": "ClueRevealed",
      "schema_version": 1,
      "clue_id": "clue_wrong_signature",
      "visibility": "party_visible"
    }
  ],
  "append_tests": [
    {
      "case_id": "append_expected_version_ok",
      "expected_version": 6,
      "append": "SessionSummaryCreated",
      "event_schema_version": 1,
      "expect": "ALLOW"
    },
    {
      "case_id": "append_version_conflict",
      "expected_version": 5,
      "append": "SessionSummaryCreated",
      "event_schema_version": 1,
      "expect": "VERSION_CONFLICT"
    },
    {
      "case_id": "idempotency_repeat",
      "idempotency_key": "idem_repeat",
      "append_twice": true,
      "expect": "SECOND_RETURNS_EXISTING"
    }
  ]
}
```
