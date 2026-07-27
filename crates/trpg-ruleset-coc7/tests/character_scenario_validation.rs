use std::collections::BTreeMap;

use trpg_ruleset_coc7::character_combat_san_chase::{
    parse_scenario_json, parse_scenario_yaml, CharacterScenarioError, Coc7CharacterSheet,
    Coc7Characteristics,
};

const TUTORIAL_SCENARIO: &str =
    include_str!("../../../fixtures/scenarios/tutorial_mist_archive.scenario.yaml");

fn valid_character() -> Coc7CharacterSheet {
    Coc7CharacterSheet {
        name: "Evelyn Hart".to_owned(),
        age: 31,
        occupation: "Investigative journalist".to_owned(),
        era: "1920s".to_owned(),
        birthplace: "Brisbane".to_owned(),
        characteristics: Coc7Characteristics {
            strength: 50,
            dexterity: 60,
            power: 65,
            constitution: 55,
            size: 50,
            appearance: 55,
            intelligence: 70,
            education: 75,
            luck: 60,
        },
        skills: BTreeMap::from([
            ("Library Use".to_owned(), 70),
            ("Psychology".to_owned(), 55),
        ]),
        backstory_anchors: vec![
            "Protects confidential sources".to_owned(),
            "Distrusts official explanations".to_owned(),
        ],
    }
}

#[test]
fn raw_tutorial_yaml_is_valid_and_round_trips_as_stable_json() {
    assert!(!TUTORIAL_SCENARIO.contains("```"));

    let yaml = parse_scenario_yaml(TUTORIAL_SCENARIO).expect("raw scenario YAML must parse");
    assert_eq!(yaml.scenario_id, "tutorial_mist_archive");
    assert_eq!(yaml.ruleset_id, "coc7");
    assert_eq!(yaml.opening_scene_id, "scene_archive_front");
    assert_eq!(
        yaml.scene_ids,
        vec!["scene_archive_front", "scene_basement"]
    );
    assert!(yaml.content_hash.starts_with("sha256:"));

    let json = parse_scenario_json(&yaml.canonical_json).expect("canonical JSON must parse");
    assert_eq!(json.content_hash, yaml.content_hash);
    assert_eq!(json.canonical_json, yaml.canonical_json);
}

#[test]
fn core_clues_need_two_independent_acquisition_paths() {
    let invalid = TUTORIAL_SCENARIO.replace("      - ask npc_marta with Psychology normal\n", "");
    assert_eq!(
        parse_scenario_yaml(&invalid),
        Err(CharacterScenarioError::CoreClueHasSinglePath(
            "clue_wrong_signature".to_owned()
        ))
    );
}

#[test]
fn encounters_reject_duplicate_or_unusable_participants() {
    for invalid in [
        TUTORIAL_SCENARIO.replace(
            "participants: [investigator, npc_marta]",
            "participants: [npc_marta, npc_marta]",
        ),
        TUTORIAL_SCENARIO.replacen(
            "participants: [investigator, npc_marta]",
            "participants: [investigator, \"npc marta\"]",
            1,
        ),
        TUTORIAL_SCENARIO.replace(
            "  - id: encounter_archive_escape\n    type: chase\n    scene_id: scene_basement\n    participants: [investigator, npc_marta]",
            "  - id: encounter_archive_escape\n    type: chase\n    scene_id: scene_basement\n    participants: [investigator, \"npc!marta\"]",
        ),
    ] {
        assert_eq!(
            parse_scenario_yaml(&invalid),
            Err(CharacterScenarioError::InvalidScenarioField("encounters"))
        );
    }
}

#[test]
fn endings_reject_unselectable_whitespace_padded_ids_and_awards() {
    for invalid in [
        TUTORIAL_SCENARIO.replace(
            "  - id: ending_expose_marta",
            "  - id: \" ending_expose_marta \"",
        ),
        TUTORIAL_SCENARIO.replace(
            "      - skill_name: Library Use",
            "      - skill_name: \" Library Use \"",
        ),
        TUTORIAL_SCENARIO.replace(
            "      - skill_name: Psychology",
            "      - skill_name: Library Use",
        ),
    ] {
        assert_eq!(
            parse_scenario_yaml(&invalid),
            Err(CharacterScenarioError::InvalidScenarioField("endings"))
        );
    }
}

#[test]
fn character_sheet_validation_rejects_invalid_coc7_data() {
    let valid = valid_character();
    let derived = valid.validate().expect("valid COC7 sheet");
    assert_eq!(derived.sanity, 65);
    assert_eq!(
        valid.to_canonical_json().expect("canonical character JSON"),
        serde_json::to_string(&valid).expect("serialize character")
    );

    let mut invalid = valid;
    invalid.age = 12;
    assert_eq!(
        invalid.validate(),
        Err(CharacterScenarioError::InvalidCharacterField("age"))
    );
}
