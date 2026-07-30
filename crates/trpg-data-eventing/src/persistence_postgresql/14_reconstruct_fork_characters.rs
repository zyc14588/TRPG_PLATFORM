
fn reconstruct_fork_characters(
    replay_events: &[CanonicalReplayEvent],
    campaign_id: &str,
    source_event_sequences: &BTreeSet<i64>,
) -> Result<Vec<ForkSnapshotCharacter>, CoreDomainRepositoryError> {
    let mut characters = BTreeMap::<String, ForkSnapshotCharacter>::new();
    let mut action_characters = BTreeMap::<String, String>::new();
    for replay in replay_events
        .iter()
        .filter(|event| source_event_sequences.contains(&event.sequence))
    {
        if replay.campaign_id != campaign_id {
            return Err(CoreDomainRepositoryError::Integrity(
                "fork_character_replay_campaign",
            ));
        }
        if matches!(
            replay.event_type.as_str(),
            "CharacterCreated"
                | "CharacterUpdated"
                | "CharacterSubmitted"
                | "CharacterInitialVersionApproved"
                | "PlayerActionSubmitted"
                | "SanityLossApplied"
                | "CombatStateRecorded"
                | "CharacterGrowthApplied"
                | "CampaignForkMaterialized"
        ) && (replay.integrity_status != "verified_hmac"
            || replay.request_hash_source != "formal_commit"
            || replay.event_integrity_hash.is_none())
        {
            return Err(CoreDomainRepositoryError::Integrity(
                "fork_character_replay_unverified",
            ));
        }
        match replay.event_type.as_str() {
            "CharacterCreated" => {
                let event: CoreDomainEvent = serde_json::from_value(replay.payload.clone())
                    .map_err(|_| {
                        CoreDomainRepositoryError::Integrity("fork_character_create_payload")
                    })?;
                let CoreDomainEvent::CharacterCreated {
                    character_id,
                    campaign_id: event_campaign_id,
                    owner_user_id,
                    display_name,
                    sheet_json,
                    ..
                } = event
                else {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "fork_character_create_event",
                    ));
                };
                if event_campaign_id != campaign_id || characters.contains_key(&character_id) {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "fork_character_create_chain",
                    ));
                }
                let sheet_json: Value = serde_json::from_str(&sheet_json).map_err(|_| {
                    CoreDomainRepositoryError::Integrity("fork_character_sheet_payload")
                })?;
                if !sheet_json.is_object() {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "fork_character_sheet_shape",
                    ));
                }
                characters.insert(
                    character_id.clone(),
                    ForkSnapshotCharacter {
                        character_id,
                        owner_user_id,
                        display_name,
                        state: "DRAFT".to_owned(),
                        initial_version_locked: false,
                        visibility_label: replay.visibility_label.clone(),
                        visibility_subject: replay.visibility_subject.clone(),
                        current_sheet: Some(ForkSnapshotSheet {
                            sheet_json,
                            locked: false,
                            visibility_label: replay.visibility_label.clone(),
                            visibility_subject: replay.visibility_subject.clone(),
                        }),
                    },
                );
            }
            "CharacterUpdated" => {
                let event: CoreDomainEvent = serde_json::from_value(replay.payload.clone())
                    .map_err(|_| {
                        CoreDomainRepositoryError::Integrity("fork_character_update_payload")
                    })?;
                let CoreDomainEvent::CharacterUpdated {
                    character_id,
                    campaign_id: event_campaign_id,
                    display_name,
                    sheet_json,
                    ..
                } = event
                else {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "fork_character_update_event",
                    ));
                };
                if event_campaign_id != campaign_id {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "fork_character_update_campaign",
                    ));
                }
                let character = characters.get_mut(&character_id).ok_or(
                    CoreDomainRepositoryError::Integrity("fork_character_update_chain"),
                )?;
                if character.state != "DRAFT" || character.initial_version_locked {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "fork_character_update_locked",
                    ));
                }
                let sheet_json: Value = serde_json::from_str(&sheet_json).map_err(|_| {
                    CoreDomainRepositoryError::Integrity("fork_character_update_sheet")
                })?;
                if !sheet_json.is_object() {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "fork_character_update_sheet",
                    ));
                }
                character.display_name = display_name;
                character.visibility_label = replay.visibility_label.clone();
                character.visibility_subject = replay.visibility_subject.clone();
                character.current_sheet = Some(ForkSnapshotSheet {
                    sheet_json,
                    locked: false,
                    visibility_label: replay.visibility_label.clone(),
                    visibility_subject: replay.visibility_subject.clone(),
                });
            }
            "CharacterSubmitted" | "CharacterInitialVersionApproved" => {
                let event: CoreDomainEvent = serde_json::from_value(replay.payload.clone())
                    .map_err(|_| {
                        CoreDomainRepositoryError::Integrity("fork_character_state_payload")
                    })?;
                let (character_id, approved) = match event {
                    CoreDomainEvent::CharacterSubmitted { character_id, .. } => {
                        (character_id, false)
                    }
                    CoreDomainEvent::CharacterInitialVersionApproved { character_id, .. } => {
                        (character_id, true)
                    }
                    _ => {
                        return Err(CoreDomainRepositoryError::Integrity(
                            "fork_character_state_event",
                        ))
                    }
                };
                let character = characters.get_mut(&character_id).ok_or(
                    CoreDomainRepositoryError::Integrity("fork_character_state_chain"),
                )?;
                character.state = if approved { "APPROVED" } else { "SUBMITTED" }.to_owned();
                character.visibility_label = replay.visibility_label.clone();
                character.visibility_subject = replay.visibility_subject.clone();
                if approved {
                    character.initial_version_locked = true;
                    let sheet = character.current_sheet.as_mut().ok_or(
                        CoreDomainRepositoryError::Integrity("fork_character_sheet_missing"),
                    )?;
                    sheet.locked = true;
                    sheet.visibility_label = replay.visibility_label.clone();
                    sheet.visibility_subject = replay.visibility_subject.clone();
                }
            }
            "PlayerActionSubmitted" => {
                let action_id = replay
                    .payload
                    .get("action_id")
                    .and_then(Value::as_str)
                    .ok_or(CoreDomainRepositoryError::Integrity(
                        "fork_player_action_id",
                    ))?;
                let character_id = replay
                    .payload
                    .get("character_id")
                    .and_then(Value::as_str)
                    .ok_or(CoreDomainRepositoryError::Integrity(
                        "fork_player_action_character",
                    ))?;
                action_characters.insert(action_id.to_owned(), character_id.to_owned());
            }
            "SanityLossApplied" => {
                let action_id = replay
                    .payload
                    .get("action_id")
                    .and_then(Value::as_str)
                    .ok_or(CoreDomainRepositoryError::Integrity(
                        "fork_sanity_action_id",
                    ))?;
                let character_id = action_characters.get(action_id).ok_or(
                    CoreDomainRepositoryError::Integrity("fork_sanity_action_chain"),
                )?;
                let character = characters.get_mut(character_id).ok_or(
                    CoreDomainRepositoryError::Integrity("fork_sanity_character_chain"),
                )?;
                let sheet = character.current_sheet.as_mut().ok_or(
                    CoreDomainRepositoryError::Integrity("fork_sanity_sheet_missing"),
                )?;
                let number = |field: &'static str| {
                    replay
                        .payload
                        .get(field)
                        .and_then(Value::as_u64)
                        .ok_or(CoreDomainRepositoryError::Integrity("fork_sanity_payload"))
                };
                let day_key = replay
                    .payload
                    .get("day_key")
                    .and_then(Value::as_str)
                    .ok_or(CoreDomainRepositoryError::Integrity("fork_sanity_payload"))?;
                let madness_state = replay
                    .payload
                    .get("madness_state")
                    .and_then(Value::as_str)
                    .ok_or(CoreDomainRepositoryError::Integrity("fork_sanity_payload"))?;
                let sanity_before = number("sanity_before")?;
                let prior_sanity = sheet
                    .sheet_json
                    .pointer("/sanity_state/current_sanity")
                    .and_then(Value::as_u64)
                    .or_else(|| {
                        sheet
                            .sheet_json
                            .pointer("/characteristics/power")
                            .and_then(Value::as_u64)
                    })
                    .ok_or(CoreDomainRepositoryError::Integrity("fork_sanity_source"))?;
                if prior_sanity != sanity_before {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "fork_sanity_source_mismatch",
                    ));
                }
                sheet.sheet_json["sanity_state"] = serde_json::json!({
                    "day_key": day_key,
                    "day_start_sanity": number("day_start_sanity")?,
                    "current_sanity": number("sanity_after")?,
                    "day_loss": number("day_loss")?,
                    "madness_state": madness_state,
                });
                sheet.locked = true;
                sheet.visibility_label = replay.visibility_label.clone();
                sheet.visibility_subject = replay.visibility_subject.clone();
                character.visibility_label = replay.visibility_label.clone();
                character.visibility_subject = replay.visibility_subject.clone();
            }
            "CombatStateRecorded" => {
                let event: CoreDomainEvent = serde_json::from_value(replay.payload.clone())
                    .map_err(|_| {
                        CoreDomainRepositoryError::Integrity("fork_combat_health_payload")
                    })?;
                let CoreDomainEvent::CombatStateRecorded {
                    campaign_id: event_campaign_id,
                    state_json,
                    character_health_updates,
                    ..
                } = event
                else {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "fork_combat_health_event",
                    ));
                };
                if event_campaign_id != campaign_id {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "fork_combat_health_campaign",
                    ));
                }
                let participants = combat_participant_values(&state_json)?;
                for update in character_health_updates {
                    let participant = participants.get(&update.character_id).ok_or(
                        CoreDomainRepositoryError::Integrity("fork_combat_health_participant"),
                    )?;
                    if participant.get("current_hp").and_then(Value::as_u64)
                        != Some(u64::from(update.hp_after))
                        || participant.get("condition").and_then(Value::as_str)
                            != Some(update.condition_after.as_str())
                    {
                        return Err(CoreDomainRepositoryError::Integrity(
                            "fork_combat_health_participant",
                        ));
                    }
                    let character = characters.get_mut(&update.character_id).ok_or(
                        CoreDomainRepositoryError::Integrity("fork_combat_health_character_chain"),
                    )?;
                    let sheet = character.current_sheet.as_mut().ok_or(
                        CoreDomainRepositoryError::Integrity("fork_combat_health_sheet_missing"),
                    )?;
                    let profile = sheet
                        .sheet_json
                        .get_mut("combat_profile")
                        .and_then(Value::as_object_mut)
                        .ok_or(CoreDomainRepositoryError::Integrity(
                            "fork_combat_health_profile",
                        ))?;
                    profile.insert("current_hp".to_owned(), Value::from(update.hp_after));
                    profile.insert(
                        "condition".to_owned(),
                        Value::String(update.condition_after),
                    );
                    sheet.locked = true;
                }
            }
            "CharacterGrowthApplied" => {
                let event: CoreDomainEvent = serde_json::from_value(replay.payload.clone())
                    .map_err(|_| CoreDomainRepositoryError::Integrity("fork_growth_payload"))?;
                let CoreDomainEvent::CharacterGrowthApplied {
                    campaign_id: event_campaign_id,
                    character_id,
                    skill_name,
                    skill_before,
                    skill_after,
                    ..
                } = event
                else {
                    return Err(CoreDomainRepositoryError::Integrity("fork_growth_event"));
                };
                if event_campaign_id != campaign_id {
                    return Err(CoreDomainRepositoryError::Integrity("fork_growth_campaign"));
                }
                let character = characters.get_mut(&character_id).ok_or(
                    CoreDomainRepositoryError::Integrity("fork_growth_character_chain"),
                )?;
                let sheet = character.current_sheet.as_mut().ok_or(
                    CoreDomainRepositoryError::Integrity("fork_growth_sheet_missing"),
                )?;
                let skill = sheet
                    .sheet_json
                    .get_mut("skills")
                    .and_then(Value::as_object_mut)
                    .ok_or(CoreDomainRepositoryError::Integrity(
                        "fork_growth_skills_missing",
                    ))?;
                if skill.get(&skill_name).and_then(Value::as_u64) != Some(u64::from(skill_before)) {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "fork_growth_source_mismatch",
                    ));
                }
                skill.insert(skill_name.clone(), Value::from(skill_after));
                sync_combat_skill_target(
                    &mut sheet.sheet_json,
                    &skill_name,
                    skill_before,
                    skill_after,
                )?;
                sheet.locked = true;
                sheet.visibility_label = replay.visibility_label.clone();
                sheet.visibility_subject = replay.visibility_subject.clone();
                character.visibility_label = replay.visibility_label.clone();
                character.visibility_subject = replay.visibility_subject.clone();
            }
            "CampaignForkMaterialized" => {
                let event: CoreDomainEvent = serde_json::from_value(replay.payload.clone())
                    .map_err(|_| {
                        CoreDomainRepositoryError::Integrity("fork_nested_materialization_payload")
                    })?;
                let CoreDomainEvent::CampaignForkMaterialized {
                    child_campaign_id,
                    rows,
                    ..
                } = event
                else {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "fork_nested_materialization_event",
                    ));
                };
                if child_campaign_id != campaign_id {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "fork_nested_materialization_campaign",
                    ));
                }
                for row in rows {
                    let CampaignForkMaterializedRow::Character {
                        character_id,
                        owner_user_id,
                        display_name,
                        state,
                        initial_version_locked,
                        sheet_json,
                        sheet_locked,
                        visibility_label,
                        visibility_subject,
                        ..
                    } = row
                    else {
                        continue;
                    };
                    if visibility_label != replay.visibility_label
                        || visibility_subject != replay.visibility_subject
                        || characters.contains_key(&character_id)
                    {
                        return Err(CoreDomainRepositoryError::Integrity(
                            "fork_nested_character_chain",
                        ));
                    }
                    let sheet_json: Value = serde_json::from_str(&sheet_json).map_err(|_| {
                        CoreDomainRepositoryError::Integrity("fork_nested_character_sheet")
                    })?;
                    characters.insert(
                        character_id.clone(),
                        ForkSnapshotCharacter {
                            character_id,
                            owner_user_id,
                            display_name,
                            state,
                            initial_version_locked,
                            visibility_label: visibility_label.clone(),
                            visibility_subject: visibility_subject.clone(),
                            current_sheet: Some(ForkSnapshotSheet {
                                sheet_json,
                                locked: sheet_locked,
                                visibility_label,
                                visibility_subject,
                            }),
                        },
                    );
                }
            }
            _ => {}
        }
    }
    characters.retain(|_, character| {
        fork_character_visibility_is_copyable(
            &character.visibility_label,
            &character.visibility_subject,
            &character.owner_user_id,
        ) && character.current_sheet.as_ref().is_some_and(|sheet| {
            fork_character_visibility_is_copyable(
                &sheet.visibility_label,
                &sheet.visibility_subject,
                &character.owner_user_id,
            )
        })
    });
    Ok(characters.into_values().collect())
}
