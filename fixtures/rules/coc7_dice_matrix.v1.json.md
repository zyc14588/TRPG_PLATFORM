# COC7 Dice Matrix v1

```json
{
  "skill_checks": [
    {
      "id": "normal_success",
      "skill_value": 60,
      "difficulty": "normal",
      "roll": 37,
      "expected_level": "regular_success"
    },
    {
      "id": "hard_success",
      "skill_value": 60,
      "difficulty": "hard",
      "roll": 30,
      "expected_level": "hard_success"
    },
    {
      "id": "extreme_success",
      "skill_value": 60,
      "difficulty": "extreme",
      "roll": 12,
      "expected_level": "extreme_success"
    },
    {
      "id": "fumble_candidate_high_skill",
      "skill_value": 60,
      "difficulty": "normal",
      "roll": 100,
      "expected_level": "fumble"
    }
  ],
  "bonus_penalty": [
    {
      "id": "one_bonus_die",
      "ones": 7,
      "tens": [
        80,
        30
      ],
      "bonus_dice": 1,
      "expected_final": 37
    },
    {
      "id": "one_penalty_die",
      "ones": 7,
      "tens": [
        30,
        80
      ],
      "penalty_dice": 1,
      "expected_final": 87
    }
  ],
  "server_roll_required": {
    "formal_roll_sources_allowed": [
      "server_rng"
    ],
    "formal_roll_sources_denied": [
      "frontend",
      "ai_text",
      "player_submitted_number"
    ]
  }
}
```
