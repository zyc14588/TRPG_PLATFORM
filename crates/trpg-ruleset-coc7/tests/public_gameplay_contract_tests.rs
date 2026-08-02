use serde_json::json;
use trpg_ruleset_coc7::coc7_rules_engine::{
    resolve_public_gameplay, PublicGameplayAction, PublicGameplayContext,
};

fn context() -> PublicGameplayContext {
    PublicGameplayContext {
        npc_public_identity: "档案管理员玛塔".to_owned(),
        character_combat_profile: json!({
            "dexterity": 70,
            "skill_targets": {"melee": 45, "firearm": 35, "dodge": 40},
            "weapon_loadout": {
                "melee": {"damage_formula": {"dice_count": 1, "die_sides": 6, "flat_bonus": 1}},
                "firearm": {"damage_formula": {"dice_count": 1, "die_sides": 6, "flat_bonus": 5}}
            },
            "current_hp": 10,
            "max_hp": 10,
            "armor": 1,
            "condition": "ABLE"
        }),
        npc_combat_profile: json!({
            "dexterity": 80,
            "skill_targets": {"melee": 60, "firearm": 80, "dodge": 40},
            "weapon_loadout": {
                "melee": {"damage_formula": {"dice_count": 1, "die_sides": 6, "flat_bonus": 0}},
                "firearm": {"damage_formula": {"dice_count": 1, "die_sides": 6, "flat_bonus": 5}}
            },
            "current_hp": 8,
            "max_hp": 8,
            "armor": 0,
            "condition": "ABLE"
        }),
        character_chase_profile: json!({"role": "QUARRY", "movement_rate": 8}),
        npc_chase_profile: json!({"role": "PURSUER", "movement_rate": 8}),
    }
}

#[test]
fn public_gameplay_uses_existing_coc7_rules_and_server_dice() {
    let context = context();

    let npc = resolve_public_gameplay(
        &context,
        &PublicGameplayAction::NpcInteraction {
            character_id: "investigator_public".to_owned(),
            npc_id: "npc_marta".to_owned(),
            approach: "询问昨夜的访客记录".to_owned(),
            public_response: "玛塔避开视线，声称没有访客。".to_owned(),
        },
    )
    .expect("NPC interaction resolves");
    assert_eq!(npc.event_type(), "coc7.npc_decision_recorded");

    let combat = resolve_public_gameplay(
        &context,
        &PublicGameplayAction::CombatRound {
            character_id: "investigator_public".to_owned(),
            npc_id: "npc_marta".to_owned(),
            action_kind: "MELEE".to_owned(),
            defense: "DODGE".to_owned(),
        },
    )
    .expect("combat round resolves");
    assert_eq!(combat.event_type(), "CombatStateUpdated");
    let combat_json = serde_json::to_value(combat).expect("serialize combat result");
    assert_eq!(combat_json["kind"], "COMBAT_ROUND");
    assert_eq!(combat_json["random_source"], "SERVER_OS_CSPRNG");
    assert!(combat_json["attacker_roll"]["roll_id"]
        .as_str()
        .is_some_and(|value| value.starts_with("dice_")));

    let chase = resolve_public_gameplay(
        &context,
        &PublicGameplayAction::ChaseSegment {
            character_id: "investigator_public".to_owned(),
            npc_id: "npc_marta".to_owned(),
            initial_range: 2,
            obstacle_id: Some("collapsing_salt_shelf".to_owned()),
            obstacle_cost: 1,
        },
    )
    .expect("chase segment resolves");
    assert_eq!(chase.event_type(), "ChaseSegmentResolved");
    let chase_json = serde_json::to_value(chase).expect("serialize chase result");
    assert_eq!(chase_json["kind"], "CHASE_SEGMENT");
    assert_eq!(chase_json["random_source"], "SERVER_OS_CSPRNG");
    assert_eq!(chase_json["before_range"], 2);
}
