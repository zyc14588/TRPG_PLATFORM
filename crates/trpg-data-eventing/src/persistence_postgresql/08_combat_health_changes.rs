
fn combat_health_changes(
    previous_state: Option<&Value>,
    next_state: &Value,
) -> Result<Vec<CombatHealthChange>, CoreDomainRepositoryError> {
    let Some(previous_state) = previous_state else {
        return Ok(Vec::new());
    };
    let previous = combat_participant_values(
        &serde_json::to_string(previous_state)
            .map_err(|_| CoreDomainRepositoryError::Serialization)?,
    )?;
    let next = combat_participant_values(
        &serde_json::to_string(next_state).map_err(|_| CoreDomainRepositoryError::Serialization)?,
    )?;
    if previous.keys().ne(next.keys()) {
        return Err(CoreDomainRepositoryError::Integrity(
            "combat_health_participant_chain",
        ));
    }
    let health = |participant: &Value| {
        let hp = participant
            .get("current_hp")
            .and_then(Value::as_u64)
            .and_then(|value| u8::try_from(value).ok())
            .ok_or(CoreDomainRepositoryError::Integrity(
                "combat_health_participant_shape",
            ))?;
        let condition = participant
            .get("condition")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty() && value.len() <= 64)
            .ok_or(CoreDomainRepositoryError::Integrity(
                "combat_health_participant_shape",
            ))?;
        Ok::<_, CoreDomainRepositoryError>((hp, condition.to_owned()))
    };
    let mut changes = Vec::new();
    for (participant_id, previous_participant) in previous {
        let next_participant =
            next.get(&participant_id)
                .ok_or(CoreDomainRepositoryError::Integrity(
                    "combat_health_participant_chain",
                ))?;
        let (hp_before, condition_before) = health(&previous_participant)?;
        let (hp_after, condition_after) = health(next_participant)?;
        if hp_before != hp_after || condition_before != condition_after {
            changes.push(CombatHealthChange {
                character_id: participant_id,
                hp_before,
                hp_after,
                condition_before,
                condition_after,
            });
        }
    }
    if changes.len() > 1 {
        return Err(CoreDomainRepositoryError::Integrity(
            "combat_health_transition_scope",
        ));
    }
    Ok(changes)
}

fn combat_health_sheet_version_id(
    combat_id: &str,
    character_id: &str,
    combat_version: i64,
) -> Result<String, CoreDomainRepositoryError> {
    let digest = format!(
        "{:x}",
        Sha256::digest(
            format!("combat-health:{combat_id}:{character_id}:{combat_version}").as_bytes()
        )
    );
    let value = format!("combat_sheet_{}", &digest[..32]);
    EntityId::new(&value)
        .map(|id| id.as_str().to_owned())
        .map_err(|_| CoreDomainRepositoryError::Integrity("combat_health_sheet_identity"))
}

fn sync_combat_skill_target(
    sheet_json: &mut Value,
    skill_name: &str,
    skill_before: u8,
    skill_after: u8,
) -> Result<(), CoreDomainRepositoryError> {
    let Some(profile) = sheet_json
        .get_mut("combat_profile")
        .and_then(Value::as_object_mut)
    else {
        return Ok(());
    };
    let explicit_sources = profile.get("skill_target_sources").cloned();
    let targets = profile
        .get_mut("skill_targets")
        .and_then(Value::as_object_mut)
        .ok_or(CoreDomainRepositoryError::Integrity(
            "combat_skill_targets_missing",
        ))?;
    let mut matched_targets = BTreeSet::new();
    if let Some(explicit_sources) = explicit_sources {
        let sources = explicit_sources
            .as_object()
            .ok_or(CoreDomainRepositoryError::Integrity(
                "combat_skill_target_sources",
            ))?;
        for (target_name, source_name) in sources {
            if !matches!(
                target_name.as_str(),
                "melee" | "firearm" | "dodge" | "first_aid" | "medicine"
            ) || source_name
                .as_str()
                .filter(|value| !value.trim().is_empty() && value.len() <= 128)
                .is_none()
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "combat_skill_target_sources",
                ));
            }
            if source_name.as_str() == Some(skill_name) {
                matched_targets.insert(target_name.clone());
            }
        }
    } else {
        let target = match skill_name {
            "Dodge" => Some("dodge"),
            "First Aid" => Some("first_aid"),
            "Medicine" => Some("medicine"),
            "Melee" | "melee" | "Fighting" => Some("melee"),
            "Firearm" | "firearm" | "Firearms" => Some("firearm"),
            value if value.starts_with("Fighting (") && value.ends_with(')') => Some("melee"),
            value if value.starts_with("Firearms (") && value.ends_with(')') => Some("firearm"),
            _ => None,
        };
        if let Some(target) = target {
            matched_targets.insert(target.to_owned());
        }
    }
    if !matched_targets.is_empty() && !(1..=100).contains(&skill_after) {
        return Err(CoreDomainRepositoryError::Integrity(
            "combat_skill_target_value",
        ));
    }
    for target in matched_targets {
        targets
            .get(&target)
            .and_then(Value::as_u64)
            .filter(|value| *value == u64::from(skill_before))
            .ok_or(CoreDomainRepositoryError::Integrity(
                "combat_skill_target_source_missing",
            ))?;
        targets.insert(target, Value::from(skill_after));
    }
    Ok(())
}

#[cfg(test)]
mod combat_sheet_projection_tests {
    use super::*;

    #[test]
    fn growth_updates_explicit_and_legacy_combat_skill_bindings() {
        let mut explicit = serde_json::json!({
            "skills": {
                "Fighting (Brawl)": 45,
                "Dodge": 40
            },
            "combat_profile": {
                "skill_targets": {
                    "melee": 45,
                    "firearm": 35,
                    "dodge": 40,
                    "first_aid": 30,
                    "medicine": 10
                },
                "skill_target_sources": {
                    "melee": "Fighting (Brawl)",
                    "firearm": "Firearms (Handgun)",
                    "dodge": "Dodge",
                    "first_aid": "First Aid",
                    "medicine": "Medicine"
                }
            }
        });
        sync_combat_skill_target(&mut explicit, "Fighting (Brawl)", 45, 51).unwrap();
        assert_eq!(
            explicit.pointer("/combat_profile/skill_targets/melee"),
            Some(&Value::from(51))
        );

        let mut legacy = serde_json::json!({
            "skills": {"Dodge": 40},
            "combat_profile": {
                "skill_targets": {
                    "melee": 45,
                    "firearm": 35,
                    "dodge": 40,
                    "first_aid": 30,
                    "medicine": 10
                }
            }
        });
        sync_combat_skill_target(&mut legacy, "Dodge", 40, 44).unwrap();
        assert_eq!(
            legacy.pointer("/combat_profile/skill_targets/dodge"),
            Some(&Value::from(44))
        );
    }

    #[test]
    fn combat_health_change_creates_one_stable_sheet_identity() {
        let previous = serde_json::json!({
            "participants": [
                {"participant_id":"character_evelyn","current_hp":10,"condition":"ABLE"},
                {"participant_id":"npc_marta","current_hp":8,"condition":"ABLE"}
            ]
        });
        let next = serde_json::json!({
            "participants": [
                {"participant_id":"character_evelyn","current_hp":5,"condition":"MAJOR_WOUND"},
                {"participant_id":"npc_marta","current_hp":8,"condition":"ABLE"}
            ]
        });
        let changes = combat_health_changes(Some(&previous), &next).unwrap();
        assert_eq!(
            changes,
            vec![CombatHealthChange {
                character_id: "character_evelyn".to_owned(),
                hp_before: 10,
                hp_after: 5,
                condition_before: "ABLE".to_owned(),
                condition_after: "MAJOR_WOUND".to_owned(),
            }]
        );
        let first =
            combat_health_sheet_version_id("combat_basement", "character_evelyn", 2).unwrap();
        assert_eq!(
            first,
            combat_health_sheet_version_id("combat_basement", "character_evelyn", 2).unwrap()
        );
        assert_ne!(
            first,
            combat_health_sheet_version_id("combat_basement", "character_evelyn", 3).unwrap()
        );
    }
}

#[derive(Clone, Debug)]
struct CanonicalCombatParticipantSnapshot {
    combat_id: String,
    status: String,
    participant: Value,
    sequence: i64,
}

#[derive(Clone, Debug)]
struct CanonicalCombatParticipantHistory {
    initial_same_combat: Option<BTreeMap<String, Value>>,
    latest: BTreeMap<String, CanonicalCombatParticipantSnapshot>,
}

fn combat_participant_values(
    state_json: &str,
) -> Result<BTreeMap<String, Value>, CoreDomainRepositoryError> {
    let state: Value = serde_json::from_str(state_json)
        .map_err(|_| CoreDomainRepositoryError::Integrity("combat_participant_state"))?;
    let participants = state.get("participants").and_then(Value::as_array).ok_or(
        CoreDomainRepositoryError::Integrity("combat_participant_state"),
    )?;
    let mut indexed = BTreeMap::new();
    for participant in participants {
        let participant_id = participant
            .get("participant_id")
            .and_then(Value::as_str)
            .ok_or(CoreDomainRepositoryError::Integrity(
                "combat_participant_identity",
            ))?;
        EntityId::new(participant_id)
            .map_err(|_| CoreDomainRepositoryError::Integrity("combat_participant_identity"))?;
        if !participant.is_object()
            || indexed
                .insert(participant_id.to_owned(), participant.clone())
                .is_some()
        {
            return Err(CoreDomainRepositoryError::Integrity(
                "combat_participant_identity",
            ));
        }
    }
    Ok(indexed)
}
