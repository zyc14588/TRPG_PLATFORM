mod common;

use trpg_ruleset_coc7::character_combat_san_chase::{
    derive_character_stats, record_character_combat_san_chase_decision,
    CharacterCombatSanChaseTrack, Coc7Characteristics, DamageBonus,
};

#[test]
fn character_facade_derives_coc7_stats() {
    let stats = derive_character_stats(Coc7Characteristics {
        strength: 60,
        dexterity: 70,
        power: 55,
        constitution: 50,
        size: 60,
        appearance: 40,
        intelligence: 80,
        education: 65,
        luck: 45,
    })
    .unwrap();

    assert_eq!(stats.hit_points, 11);
    assert_eq!(stats.magic_points, 11);
    assert_eq!(stats.sanity, 55);
    assert_eq!(stats.damage_bonus, DamageBonus::None);
}

#[test]
fn character_facade_records_track_decision() {
    let contract = common::human_contract();
    let mut store = common::event_store();
    let command = common::rules_command("character");

    let event = record_character_combat_san_chase_decision(
        &contract,
        &mut store,
        &command,
        CharacterCombatSanChaseTrack::Character,
    )
    .unwrap();

    assert_eq!(event.payload.decision_type, "character_combat_san_chase");
}
