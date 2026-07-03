# COC7 SAN Combat Chase Flow v1

```json
{
  "sanity": [
    {
      "id": "san_success_no_loss",
      "san": 55,
      "roll": 41,
      "formula": "0/1d3",
      "loss_roll": 2,
      "expected_loss": 0,
      "expected_events": [
        "DiceRolled",
        "SanityCheckResolved"
      ]
    },
    {
      "id": "san_failure_loss",
      "san": 55,
      "roll": 81,
      "formula": "1/1d4",
      "loss_roll": 3,
      "expected_loss": 3,
      "expected_possible_temp_madness": false
    }
  ],
  "combat": [
    {
      "id": "ghoul_melee_hit",
      "attacker": "char_a",
      "defender": "npc_ghoul",
      "attack_skill": "FightingBrawl",
      "attack_roll": 25,
      "defender_action": "dodge",
      "dodge_roll": 61,
      "damage_formula": "1d6+1",
      "damage_roll": 4,
      "expected_damage": 5,
      "expected_events": [
        "CombatRoundAdvanced",
        "DiceRolled",
        "DamageApplied"
      ]
    }
  ],
  "chase": [
    {
      "id": "lead_increases",
      "start_lead": 2,
      "runner_success": true,
      "pursuer_success": false,
      "expected_lead": 3
    },
    {
      "id": "caught_or_melee",
      "start_lead": 1,
      "runner_success": false,
      "pursuer_success": true,
      "expected_lead": 0,
      "expected_state": "caught_or_melee"
    }
  ]
}
```
