# Test Data — COC7 Dice / SAN / Combat / Chase Cases

```json
{
  "skill_checks": [
    {"skill_value":60,"difficulty":"normal","roll":37,"expected":"success"},
    {"skill_value":60,"difficulty":"hard","roll":30,"expected":"hard_success"},
    {"skill_value":60,"difficulty":"extreme","roll":12,"expected":"extreme_success"},
    {"skill_value":60,"difficulty":"normal","roll":96,"expected":"fumble_candidate"}
  ],
  "bonus_penalty": [
    {"ones":7,"tens":[80,30],"bonus_dice":1,"expected_final":37},
    {"ones":7,"tens":[30,80],"penalty_dice":1,"expected_final":87}
  ],
  "sanity": [
    {"san":55,"roll":41,"formula":"0/1d3","loss_roll":2,"expected_loss":0},
    {"san":55,"roll":81,"formula":"1/1d4","loss_roll":3,"expected_loss":3,"possible_temp_madness":false}
  ],
  "combat": [
    {"attacker":"char_a","defender":"npc_ghoul","attack_roll":25,"dodge_roll":61,"damage":"1d6+1","expected":"hit_damage_applied"}
  ],
  "chase": [
    {"lead":2,"runner_success":true,"pursuer_success":false,"expected_lead":3},
    {"lead":1,"runner_success":false,"pursuer_success":true,"expected_lead":0,"expected_state":"caught_or_melee"}
  ]
}
```
