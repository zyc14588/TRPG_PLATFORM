
fn canonical_combat_participant_snapshots(
    replay_events: &[CanonicalReplayEvent],
    campaign_id: &str,
    combat_id: &str,
) -> Result<CanonicalCombatParticipantHistory, CoreDomainRepositoryError> {
    let mut initial_same_combat = None;
    let mut latest = BTreeMap::<String, CanonicalCombatParticipantSnapshot>::new();
    for replay in replay_events
        .iter()
        .filter(|event| event.event_type == "CombatStateRecorded")
    {
        let event: CoreDomainEvent = serde_json::from_value(replay.payload.clone())
            .map_err(|_| CoreDomainRepositoryError::Integrity("combat_history_payload"))?;
        event.validate_schema_version()?;
        let CoreDomainEvent::CombatStateRecorded {
            combat_id: recorded_combat_id,
            campaign_id: recorded_campaign_id,
            status,
            version,
            state_json,
            ..
        } = event
        else {
            return Err(CoreDomainRepositoryError::Integrity(
                "combat_history_event_type",
            ));
        };
        if recorded_campaign_id != campaign_id {
            return Err(CoreDomainRepositoryError::Integrity(
                "combat_history_campaign",
            ));
        }
        let inspected = inspect_combat_state(&state_json)
            .map_err(|_| CoreDomainRepositoryError::Integrity("combat_history_state"))?;
        if inspected.combat_id() != recorded_combat_id
            || inspected.status() != status
            || inspected.version() != version
        {
            return Err(CoreDomainRepositoryError::Integrity("combat_history_state"));
        }
        let participants = combat_participant_values(&state_json)?;
        if recorded_combat_id == combat_id
            && version == 1
            && initial_same_combat.replace(participants.clone()).is_some()
        {
            return Err(CoreDomainRepositoryError::Integrity(
                "combat_initial_history_conflict",
            ));
        }
        for (participant_id, participant) in participants {
            let replace = latest
                .get(&participant_id)
                .is_none_or(|snapshot| replay.sequence > snapshot.sequence);
            if replace {
                latest.insert(
                    participant_id,
                    CanonicalCombatParticipantSnapshot {
                        combat_id: recorded_combat_id.clone(),
                        status: status.clone(),
                        participant,
                        sequence: replay.sequence,
                    },
                );
            }
        }
    }
    Ok(CanonicalCombatParticipantHistory {
        initial_same_combat,
        latest,
    })
}

fn scenario_combat_authorizes_participants(
    document: &Value,
    active_scene_key: &str,
    participant_ids: &BTreeSet<String>,
    character_ids: &BTreeSet<String>,
) -> bool {
    document
        .get("encounters")
        .and_then(Value::as_array)
        .is_some_and(|encounters| {
            encounters.iter().any(|encounter| {
                if encounter.get("type").and_then(Value::as_str) != Some("combat")
                    || encounter.get("scene_id").and_then(Value::as_str) != Some(active_scene_key)
                {
                    return false;
                }
                let Some(tokens) = encounter.get("participants").and_then(Value::as_array) else {
                    return false;
                };
                let mut permits_investigators = false;
                let mut explicit = BTreeSet::new();
                for token in tokens {
                    let Some(token) = token.as_str() else {
                        return false;
                    };
                    if token == "investigator" {
                        permits_investigators = true;
                    } else {
                        explicit.insert(token.to_owned());
                    }
                }
                if explicit.iter().any(|id| !participant_ids.contains(id))
                    || participant_ids.iter().any(|id| {
                        !(explicit.contains(id)
                            || permits_investigators && character_ids.contains(id))
                    })
                {
                    return false;
                }
                !permits_investigators
                    || participant_ids.iter().any(|id| character_ids.contains(id))
            })
        })
}

fn chase_participant_values(
    state_json: &str,
) -> Result<BTreeMap<String, Value>, CoreDomainRepositoryError> {
    let state: Value = serde_json::from_str(state_json)
        .map_err(|_| CoreDomainRepositoryError::Integrity("chase_participant_state"))?;
    let participants = state.get("participants").and_then(Value::as_array).ok_or(
        CoreDomainRepositoryError::Integrity("chase_participant_state"),
    )?;
    let mut indexed = BTreeMap::new();
    for participant in participants {
        let participant_id = participant
            .get("participant_id")
            .and_then(Value::as_str)
            .ok_or(CoreDomainRepositoryError::Integrity(
                "chase_participant_identity",
            ))?;
        EntityId::new(participant_id)
            .map_err(|_| CoreDomainRepositoryError::Integrity("chase_participant_identity"))?;
        if !participant.is_object()
            || indexed
                .insert(participant_id.to_owned(), participant.clone())
                .is_some()
        {
            return Err(CoreDomainRepositoryError::Integrity(
                "chase_participant_identity",
            ));
        }
    }
    Ok(indexed)
}

fn canonical_initial_chase_state(
    replay_events: &[CanonicalReplayEvent],
    campaign_id: &str,
    chase_id: &str,
) -> Result<Option<Value>, CoreDomainRepositoryError> {
    let mut initial = None;
    for replay in replay_events
        .iter()
        .filter(|event| event.event_type == "ChaseStateRecorded")
    {
        let event: CoreDomainEvent = serde_json::from_value(replay.payload.clone())
            .map_err(|_| CoreDomainRepositoryError::Integrity("chase_history_payload"))?;
        event.validate_schema_version()?;
        let CoreDomainEvent::ChaseStateRecorded {
            chase_id: recorded_chase_id,
            campaign_id: recorded_campaign_id,
            status,
            range_band,
            segment,
            version,
            state_json,
            ..
        } = event
        else {
            return Err(CoreDomainRepositoryError::Integrity(
                "chase_history_event_type",
            ));
        };
        if recorded_campaign_id != campaign_id {
            return Err(CoreDomainRepositoryError::Integrity(
                "chase_history_campaign",
            ));
        }
        let inspected = inspect_chase_state(&state_json)
            .map_err(|_| CoreDomainRepositoryError::Integrity("chase_history_state"))?;
        if inspected.chase_id() != recorded_chase_id
            || inspected.status() != status
            || u8::try_from(inspected.range()).ok() != Some(range_band)
            || u64::from(inspected.segment()) != segment
            || inspected.version() != version
        {
            return Err(CoreDomainRepositoryError::Integrity("chase_history_state"));
        }
        if recorded_chase_id == chase_id && version == 1 {
            let state: Value = serde_json::from_str(&state_json)
                .map_err(|_| CoreDomainRepositoryError::Integrity("chase_history_state"))?;
            if initial.replace(state).is_some() {
                return Err(CoreDomainRepositoryError::Integrity(
                    "chase_initial_history_conflict",
                ));
            }
        }
    }
    Ok(initial)
}

fn scenario_chase_authorizes_participants(
    document: &Value,
    active_scene_key: &str,
    participant_ids: &BTreeSet<String>,
    character_ids: &BTreeSet<String>,
    initial_range: i8,
) -> bool {
    document
        .get("encounters")
        .and_then(Value::as_array)
        .is_some_and(|encounters| {
            encounters.iter().any(|encounter| {
                if encounter.get("type").and_then(Value::as_str) != Some("chase")
                    || encounter.get("scene_id").and_then(Value::as_str) != Some(active_scene_key)
                    || encounter.get("initial_range").and_then(Value::as_i64)
                        != Some(i64::from(initial_range))
                {
                    return false;
                }
                let Some(tokens) = encounter.get("participants").and_then(Value::as_array) else {
                    return false;
                };
                let mut permits_investigators = false;
                let mut explicit = BTreeSet::new();
                for token in tokens {
                    let Some(token) = token.as_str() else {
                        return false;
                    };
                    if token == "investigator" {
                        permits_investigators = true;
                    } else {
                        explicit.insert(token.to_owned());
                    }
                }
                if explicit.iter().any(|id| !participant_ids.contains(id))
                    || participant_ids.iter().any(|id| {
                        !(explicit.contains(id)
                            || permits_investigators && character_ids.contains(id))
                    })
                {
                    return false;
                }
                !permits_investigators
                    || participant_ids.iter().any(|id| character_ids.contains(id))
            })
        })
}

fn combat_profile_participant(
    participant_id: &str,
    profile: &Value,
) -> Result<Value, CoreDomainRepositoryError> {
    let mut profile = profile
        .as_object()
        .cloned()
        .ok_or(CoreDomainRepositoryError::Integrity(
            "combat_participant_profile",
        ))?;
    if profile.contains_key("participant_id") {
        return Err(CoreDomainRepositoryError::Integrity(
            "combat_participant_profile",
        ));
    }
    // The source binding belongs to the character sheet projection. Combat
    // snapshots carry only the resolved numeric targets used by the rules
    // engine, so growth can update the binding without changing the wire
    // shape of a canonical combat participant.
    profile.remove("skill_target_sources");
    profile.insert(
        "participant_id".to_owned(),
        Value::String(participant_id.to_owned()),
    );
    Ok(Value::Object(profile))
}

fn chase_profile_participant(
    participant_id: &str,
    profile: &Value,
) -> Result<Value, CoreDomainRepositoryError> {
    let mut profile = profile
        .as_object()
        .cloned()
        .ok_or(CoreDomainRepositoryError::Integrity(
            "chase_participant_profile",
        ))?;
    if profile.contains_key("participant_id") {
        return Err(CoreDomainRepositoryError::Integrity(
            "chase_participant_profile",
        ));
    }
    profile.insert(
        "participant_id".to_owned(),
        Value::String(participant_id.to_owned()),
    );
    Ok(Value::Object(profile))
}
