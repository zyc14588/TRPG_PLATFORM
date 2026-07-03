# Test Data — Tutorial Scenario YAML

```yaml
metadata:
  scenario_id: tutorial_mist_archive
  title: 灰港档案室
  ruleset_id: coc7
  version: 0.1.0
  recommended_players: 1-3
  era: 1920s
  copyright_status: original
  safety_notes:
    - mild horror
    - no graphic gore
keeper_truth:
  summary: 一名档案管理员伪造了失踪案记录，以掩盖地下盐窖中的仪式痕迹。
  visibility: keeper_only
scenes:
  - id: scene_archive_front
    name: 灰港市政档案室前厅
    time: 18:20
    atmosphere: 雨水拍打窗框，煤气灯忽明忽暗。
    visible_npcs: [npc_marta]
    investigable_objects:
      - id: obj_guestbook
        name: 访客登记簿
        default_visibility: public
        clues: [clue_wrong_signature]
      - id: obj_locked_drawer
        name: 上锁抽屉
        default_visibility: public
        checks:
          - skill: Locksmith
            difficulty: normal
            fail_forward: clue_drawer_scratches_with_time_cost
        clues: [clue_salt_receipt]
    exits: [scene_basement]
  - id: scene_basement
    name: 地下盐窖
    time: 19:00
    atmosphere: 空气里有潮湿盐粒和旧纸张的味道。
    dangers: [danger_sanity_symbol]
    clues: [clue_chalk_circle]
npcs:
  - id: npc_marta
    public_identity: 档案管理员玛塔
    true_identity: 仪式协助者
    attitude: nervous
    lies:
      - 她声称昨晚没有人进出地下室。
    revealable_clues: [clue_wrong_signature, clue_salt_receipt]
clues:
  - id: clue_wrong_signature
    type: core
    state: discoverable
    visibility: party_visible
    acquisition:
      - inspect obj_guestbook
      - ask npc_marta with Psychology normal
  - id: clue_salt_receipt
    type: core
    state: discoverable
    visibility: party_visible
    acquisition:
      - open obj_locked_drawer
      - fail_forward after two stalled turns
  - id: clue_chalk_circle
    type: mythos
    state: discoverable
    visibility: party_visible
    triggers:
      - enter scene_basement
    sanity_check:
      formula: 0/1d3
      difficulty: normal
timeline:
  - id: timer_archive_closing
    starts_at: 18:20
    fires_at: 20:00
    effect: Marta attempts to lock the investigators inside.
endings:
  - id: ending_expose_marta
    condition: clue_wrong_signature and clue_salt_receipt discovered
  - id: ending_salt_cellar_escape
    condition: scene_basement entered and danger resolved
```
