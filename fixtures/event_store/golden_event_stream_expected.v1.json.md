# Golden Event Stream Expected v1

```json
{
  "stream_id": "campaign-camp_ai_harbor",
  "initial_version": 0,
  "events": [
    {
      "version": 1,
      "type": "CampaignCreated",
      "authority_mode": "AI_KP",
      "visibility": "system_only"
    },
    {
      "version": 2,
      "type": "AuthorityContractLocked",
      "locked": true,
      "visibility": "system_only"
    },
    {
      "version": 3,
      "type": "CharacterSheetSubmitted",
      "character_id": "char_a",
      "visibility": "private_to_player:user_player_a"
    },
    {
      "version": 4,
      "type": "CharacterSheetVersionLocked",
      "character_id": "char_a",
      "visibility": "private_to_player:user_player_a"
    },
    {
      "version": 5,
      "type": "DiceRolled",
      "roll_id": "roll_001",
      "formula": "1d100",
      "final_result": 37,
      "visibility": "party_visible"
    },
    {
      "version": 6,
      "type": "ClueRevealed",
      "clue_id": "clue_wrong_signature",
      "visibility": "party_visible"
    }
  ],
  "append_tests": [
    {
      "case_id": "append_expected_version_ok",
      "expected_version": 6,
      "append": "SessionSummaryCreated",
      "expect": "ALLOW"
    },
    {
      "case_id": "append_version_conflict",
      "expected_version": 5,
      "append": "SessionSummaryCreated",
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
