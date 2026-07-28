
fn validate_scenario(
    document: ScenarioDocument,
) -> Result<ValidatedScenario, CharacterScenarioError> {
    let metadata = &document.metadata;
    for (field, value) in [
        ("metadata.scenario_id", metadata.scenario_id.as_str()),
        ("metadata.title", metadata.title.as_str()),
        ("metadata.version", metadata.version.as_str()),
        (
            "metadata.recommended_players",
            metadata.recommended_players.as_str(),
        ),
        ("metadata.era", metadata.era.as_str()),
        (
            "metadata.copyright_status",
            metadata.copyright_status.as_str(),
        ),
        (
            "keeper_truth.summary",
            document.keeper_truth.summary.as_str(),
        ),
    ] {
        if value.trim().is_empty() {
            return Err(CharacterScenarioError::InvalidScenarioField(field));
        }
    }
    if metadata.ruleset_id != "coc7" {
        return Err(CharacterScenarioError::InvalidScenarioField(
            "metadata.ruleset_id",
        ));
    }
    if metadata.safety_notes.is_empty() {
        return Err(CharacterScenarioError::InvalidScenarioField(
            "metadata.safety_notes",
        ));
    }
    if document.keeper_truth.visibility != "keeper_only" {
        return Err(CharacterScenarioError::InvalidScenarioField(
            "keeper_truth.visibility",
        ));
    }
    if document.scenes.is_empty() {
        return Err(CharacterScenarioError::InvalidScenarioField("scenes"));
    }
    if document.clues.is_empty() {
        return Err(CharacterScenarioError::InvalidScenarioField("clues"));
    }
    if document.endings.is_empty() {
        return Err(CharacterScenarioError::InvalidScenarioField("endings"));
    }

    let mut scene_ids = HashSet::new();
    for scene in &document.scenes {
        if scene.id.trim().is_empty()
            || scene.name.trim().is_empty()
            || scene.atmosphere.trim().is_empty()
        {
            return Err(CharacterScenarioError::InvalidScenarioField("scene"));
        }
        if !scene_ids.insert(scene.id.clone()) {
            return Err(CharacterScenarioError::DuplicateScenarioId("scene"));
        }
    }

    let mut encounter_ids = HashSet::new();
    for encounter in &document.encounters {
        let unique_participants = encounter
            .participants
            .iter()
            .map(String::as_str)
            .collect::<HashSet<_>>();
        if encounter.id.trim().is_empty()
            || !encounter_ids.insert(encounter.id.clone())
            || !matches!(encounter.encounter_type.as_str(), "combat" | "chase")
            || !scene_ids.contains(&encounter.scene_id)
            || encounter.participants.len() < 2
            || unique_participants.len() != encounter.participants.len()
            || encounter
                .participants
                .iter()
                .any(|participant| !valid_encounter_participant_id(participant))
            || (encounter.encounter_type == "chase"
                && !encounter
                    .initial_range
                    .is_some_and(|range| (1..=4).contains(&range)))
            || (encounter.encounter_type == "combat" && encounter.initial_range.is_some())
        {
            return Err(CharacterScenarioError::InvalidScenarioField("encounters"));
        }
    }

    let mut ending_ids = HashSet::new();
    for ending in &document.endings {
        let unique_growth_awards = ending
            .growth_awards
            .iter()
            .map(|award| award.skill_name.as_str())
            .collect::<HashSet<_>>();
        if ending.id.trim().is_empty()
            || ending.id != ending.id.trim()
            || ending.condition.trim().is_empty()
            || !ending_ids.insert(ending.id.clone())
            || unique_growth_awards.len() != ending.growth_awards.len()
            || ending.growth_awards.iter().any(|award| {
                award.skill_name.trim().is_empty()
                    || award.skill_name != award.skill_name.trim()
                    || award.skill_name.len() > 128
                    || award.reason.trim().is_empty()
            })
        {
            return Err(CharacterScenarioError::InvalidScenarioField("endings"));
        }
    }
    for scene in &document.scenes {
        for exit in &scene.exits {
            if !scene_ids.contains(exit) {
                return Err(CharacterScenarioError::UnknownSceneExit(exit.clone()));
            }
        }
    }

    let mut clue_ids = HashSet::new();
    for clue in &document.clues {
        if clue.id.trim().is_empty() || !clue_ids.insert(clue.id.clone()) {
            return Err(CharacterScenarioError::DuplicateScenarioId("clue"));
        }
        if clue.clue_type == "core" && clue.acquisition.len() < 2 {
            return Err(CharacterScenarioError::CoreClueHasSinglePath(
                clue.id.clone(),
            ));
        }
    }
    for scene in &document.scenes {
        if scene.clues.iter().any(|clue| !clue_ids.contains(clue)) {
            return Err(CharacterScenarioError::InvalidScenarioField("scene.clues"));
        }
    }

    let canonical_value =
        serde_json::to_value(&document).map_err(|_| CharacterScenarioError::Serialization)?;
    let canonical_json = serde_json::to_string(&canonical_value)
        .map_err(|_| CharacterScenarioError::Serialization)?;
    let content_hash = format!("sha256:{:x}", Sha256::digest(canonical_json.as_bytes()));
    Ok(ValidatedScenario {
        scenario_id: metadata.scenario_id.clone(),
        ruleset_id: metadata.ruleset_id.clone(),
        format_version: metadata.version.clone(),
        content_hash,
        canonical_json,
        opening_scene_id: document.scenes[0].id.clone(),
        scene_ids: document
            .scenes
            .iter()
            .map(|scene| scene.id.clone())
            .collect(),
        encounter_ids: document
            .encounters
            .iter()
            .map(|encounter| encounter.id.clone())
            .collect(),
        ending_ids: document
            .endings
            .iter()
            .map(|ending| ending.id.clone())
            .collect(),
        growth_skills: document
            .endings
            .iter()
            .flat_map(|ending| ending.growth_awards.iter())
            .map(|award| award.skill_name.clone())
            .collect(),
    })
}

fn valid_encounter_participant_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

pub fn derive_character_stats(characteristics: Coc7Characteristics) -> KernelResult<DerivedStats> {
    let values = [
        characteristics.strength,
        characteristics.dexterity,
        characteristics.power,
        characteristics.constitution,
        characteristics.size,
        characteristics.appearance,
        characteristics.intelligence,
        characteristics.education,
        characteristics.luck,
    ];
    if values.iter().any(|value| *value == 0 || *value > 100) {
        return Err(TrpgError::InvalidConfiguration("characteristic_range"));
    }

    let physical_total = characteristics.strength as u16 + characteristics.size as u16;
    let (damage_bonus, build) = match physical_total {
        2..=64 => (DamageBonus::MinusTwo, -2),
        65..=84 => (DamageBonus::MinusOne, -1),
        85..=124 => (DamageBonus::None, 0),
        125..=164 => (DamageBonus::PlusD4, 1),
        _ => (DamageBonus::PlusD6, 2),
    };
    let movement_rate = match (
        characteristics.strength > characteristics.size,
        characteristics.dexterity > characteristics.size,
    ) {
        (true, true) => 9,
        (false, false) => 7,
        _ => 8,
    };

    Ok(DerivedStats {
        hit_points: ((characteristics.constitution as u16 + characteristics.size as u16) / 10)
            as u8,
        magic_points: characteristics.power / 5,
        sanity: characteristics.power,
        luck: characteristics.luck,
        movement_rate,
        damage_bonus,
        build,
    })
}

pub fn record_character_combat_san_chase_decision<T>(
    contract: &AuthorityContract,
    store: &mut EventStore<Coc7EventPayload>,
    command: &CommandEnvelope<T>,
    track: CharacterCombatSanChaseTrack,
) -> KernelResult<EventEnvelope<Coc7EventPayload>> {
    append_coc7_event(
        contract,
        store,
        command,
        trpg_contracts::EventType::Coc7CharacterTrackRecorded.name(),
        "character_combat_san_chase",
        format!("track={:?}", track),
    )
}
