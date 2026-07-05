# Detailed Stage Acceptance Fixture — v2.21

```json
{
  "stage": "S05",
  "purpose": "验证 COC7 骰子、检定、SAN、战斗、追逐和角色卡派生值。",
  "inputs": {
    "character": {
      "spot_hidden": 60,
      "san": 50,
      "hp": 12,
      "dex": 55
    },
    "rolls": [
      {
        "formula": "1d100",
        "seed": "golden-spot-hidden",
        "difficulty": "hard",
        "skill": 60
      },
      {
        "formula": "1d4",
        "seed": "bell"
      }
    ]
  },
  "actions": [
    {
      "id": "skill_check",
      "type": "coc7_roll",
      "expected_raw": 37
    },
    {
      "id": "san_check",
      "type": "sanity_resolution",
      "loss_formula": "1/1d4"
    },
    {
      "id": "combat_round",
      "type": "combat_resolution"
    },
    {
      "id": "chase_obstacle",
      "type": "chase_resolution"
    }
  ],
  "expected_events": [
    {
      "type": "DiceRolled",
      "raw_result": 37,
      "visibility": "public"
    },
    {
      "type": "SkillCheckResolved",
      "success_level": "hard_success"
    },
    {
      "type": "SanityLossApplied",
      "loss": 3
    },
    {
      "type": "CombatStateUpdated"
    },
    {
      "type": "ChaseSegmentResolved"
    }
  ],
  "expected_records": [
    {
      "record": "DiceRoll",
      "required_fields": [
        "roll_id",
        "formula",
        "raw_result",
        "difficulty",
        "linked_decision_id"
      ]
    },
    {
      "record": "CharacterSheetVersion",
      "required_fields": [
        "before_state",
        "after_state",
        "reason"
      ]
    }
  ],
  "expected_errors": [
    {
      "case": "client_generated_formal_roll",
      "error": "CLIENT_FORMAL_DICE_FORBIDDEN"
    },
    {
      "case": "ai_invented_roll",
      "error": "AI_DICE_FABRICATION_FORBIDDEN"
    }
  ],
  "failure_cases": [
    {
      "id": "san_loss_without_event",
      "expected_error": "STATE_CHANGE_WITHOUT_EVENT"
    }
  ],
  "required_evidence": [
    "evidence/stages/S05/coc7-rules-tests.txt",
    "evidence/stages/S05/dice-audit-tests.txt"
  ],
  "automation_target": "cargo test -p trpg-ruleset-coc7 dice sanity combat chase --all-features",
  "pass_criteria": [
    "server_dice_only",
    "success_level_correct",
    "san_loss_recorded",
    "combat_and_chase_state_evented"
  ]
}
```
