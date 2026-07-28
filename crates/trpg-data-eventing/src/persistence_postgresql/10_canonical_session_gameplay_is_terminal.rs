
fn canonical_session_gameplay_is_terminal(
    replay_events: &[CanonicalReplayEvent],
    campaign_id: &str,
    session_id: &str,
) -> Result<bool, CoreDomainRepositoryError> {
    let mut combats = BTreeMap::<String, (i64, String)>::new();
    let mut chases = BTreeMap::<String, (i64, String)>::new();
    for replay in replay_events.iter().filter(|event| {
        matches!(
            event.event_type.as_str(),
            "CombatStateRecorded" | "ChaseStateRecorded"
        )
    }) {
        let event: CoreDomainEvent = serde_json::from_value(replay.payload.clone())
            .map_err(|_| CoreDomainRepositoryError::Integrity("session_gameplay_payload"))?;
        event.validate_schema_version()?;
        match event {
            CoreDomainEvent::CombatStateRecorded {
                combat_id,
                campaign_id: recorded_campaign_id,
                session_id: recorded_session_id,
                status,
                version,
                state_json,
                ..
            } => {
                if recorded_campaign_id != campaign_id {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "session_gameplay_campaign",
                    ));
                }
                let inspected = inspect_combat_state(&state_json).map_err(|_| {
                    CoreDomainRepositoryError::Integrity("session_gameplay_combat_state")
                })?;
                if inspected.combat_id() != combat_id
                    || inspected.status() != status
                    || inspected.version() != version
                {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "session_gameplay_combat_state",
                    ));
                }
                if recorded_session_id == session_id
                    && combats
                        .get(&combat_id)
                        .is_none_or(|(sequence, _)| replay.sequence > *sequence)
                {
                    combats.insert(combat_id, (replay.sequence, status));
                }
            }
            CoreDomainEvent::ChaseStateRecorded {
                chase_id,
                campaign_id: recorded_campaign_id,
                session_id: recorded_session_id,
                status,
                range_band,
                segment,
                version,
                state_json,
                ..
            } => {
                if recorded_campaign_id != campaign_id {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "session_gameplay_campaign",
                    ));
                }
                let inspected = inspect_chase_state(&state_json).map_err(|_| {
                    CoreDomainRepositoryError::Integrity("session_gameplay_chase_state")
                })?;
                if inspected.chase_id() != chase_id
                    || inspected.status() != status
                    || u8::try_from(inspected.range()).ok() != Some(range_band)
                    || u64::from(inspected.segment()) != segment
                    || inspected.version() != version
                {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "session_gameplay_chase_state",
                    ));
                }
                if recorded_session_id == session_id
                    && chases
                        .get(&chase_id)
                        .is_none_or(|(sequence, _)| replay.sequence > *sequence)
                {
                    chases.insert(chase_id, (replay.sequence, status));
                }
            }
            _ => {
                return Err(CoreDomainRepositoryError::Integrity(
                    "session_gameplay_event_type",
                ))
            }
        }
    }
    Ok(combats.values().all(|(_, status)| status == "ENDED")
        && chases
            .values()
            .all(|(_, status)| matches!(status.as_str(), "ESCAPED" | "CAUGHT")))
}

fn gameplay_roll_id(
    value: &Value,
    field: &'static str,
) -> Result<String, CoreDomainRepositoryError> {
    let roll_id = value
        .get("roll_id")
        .and_then(Value::as_str)
        .ok_or(CoreDomainRepositoryError::Integrity(field))?;
    EntityId::new(roll_id)
        .map(|roll_id| roll_id.as_str().to_owned())
        .map_err(|_| CoreDomainRepositoryError::Integrity(field))
}

fn validate_gameplay_roll_consumptions(
    consumptions: Vec<GameplayRollConsumption>,
    field: &'static str,
) -> Result<Vec<GameplayRollConsumption>, CoreDomainRepositoryError> {
    if consumptions
        .iter()
        .map(|consumption| consumption.roll_id.as_str())
        .collect::<BTreeSet<_>>()
        .len()
        != consumptions.len()
    {
        return Err(CoreDomainRepositoryError::Integrity(field));
    }
    Ok(consumptions)
}

fn combat_gameplay_roll_consumptions(
    state_json: &str,
) -> Result<Vec<GameplayRollConsumption>, CoreDomainRepositoryError> {
    let state: Value = serde_json::from_str(state_json)
        .map_err(|_| CoreDomainRepositoryError::Integrity("combat_roll_consumption_state"))?;
    let transition = state
        .get("last_transition")
        .and_then(Value::as_object)
        .ok_or(CoreDomainRepositoryError::Integrity(
            "combat_roll_consumption_transition",
        ))?;
    let kind = transition.get("kind").and_then(Value::as_str).ok_or(
        CoreDomainRepositoryError::Integrity("combat_roll_consumption_transition"),
    )?;
    let mut consumptions = Vec::new();
    let mut push_roll =
        |key: &'static str, roll_kind: &'static str| -> Result<(), CoreDomainRepositoryError> {
            let value = transition
                .get(key)
                .ok_or(CoreDomainRepositoryError::Integrity(
                    "combat_roll_consumption_transition",
                ))?;
            if value.is_null() {
                return Ok(());
            }
            consumptions.push(GameplayRollConsumption {
                roll_id: gameplay_roll_id(value, "combat_roll_consumption_id")?,
                roll_kind,
            });
            Ok(())
        };
    match kind {
        "STARTED" | "TURN_ADVANCED" | "ENDED" => {}
        "ATTACK_MISSED" => {
            push_roll("attacker_roll", "ATTACKER_PERCENTILE")?;
            push_roll("defender_roll", "DEFENDER_PERCENTILE")?;
        }
        "DAMAGE_APPLIED" => {
            push_roll("attacker_roll", "ATTACKER_PERCENTILE")?;
            push_roll("defender_roll", "DEFENDER_PERCENTILE")?;
            push_roll("damage_roll", "DAMAGE")?;
        }
        "MAJOR_WOUND_RECOVERY_ATTEMPTED" => {
            push_roll("medical_roll", "MEDICAL_PERCENTILE")?;
        }
        _ => {
            return Err(CoreDomainRepositoryError::Integrity(
                "combat_roll_consumption_transition",
            ))
        }
    }
    validate_gameplay_roll_consumptions(consumptions, "combat_roll_consumption_reuse")
}

fn chase_gameplay_roll_consumptions(
    state_json: &str,
) -> Result<Vec<GameplayRollConsumption>, CoreDomainRepositoryError> {
    let state: Value = serde_json::from_str(state_json)
        .map_err(|_| CoreDomainRepositoryError::Integrity("chase_roll_consumption_state"))?;
    let transition = state
        .get("last_transition")
        .and_then(Value::as_object)
        .ok_or(CoreDomainRepositoryError::Integrity(
            "chase_roll_consumption_transition",
        ))?;
    let kind = transition.get("kind").and_then(Value::as_str).ok_or(
        CoreDomainRepositoryError::Integrity("chase_roll_consumption_transition"),
    )?;
    let consumptions = match kind {
        "STARTED" => Vec::new(),
        "ADVANCED" => transition
            .get("rolls")
            .and_then(Value::as_array)
            .ok_or(CoreDomainRepositoryError::Integrity(
                "chase_roll_consumption_transition",
            ))?
            .iter()
            .map(|roll| {
                Ok(GameplayRollConsumption {
                    roll_id: gameplay_roll_id(roll, "chase_roll_consumption_id")?,
                    roll_kind: "CHASE_PARTICIPANT_PERCENTILE",
                })
            })
            .collect::<Result<Vec<_>, CoreDomainRepositoryError>>()?,
        _ => {
            return Err(CoreDomainRepositoryError::Integrity(
                "chase_roll_consumption_transition",
            ))
        }
    };
    validate_gameplay_roll_consumptions(consumptions, "chase_roll_consumption_reuse")
}

fn growth_gameplay_roll_consumptions(
    server_roll_id: &str,
    increase_roll_id: Option<&str>,
) -> Result<Vec<GameplayRollConsumption>, CoreDomainRepositoryError> {
    let mut consumptions = vec![GameplayRollConsumption {
        roll_id: EntityId::new(server_roll_id)
            .map_err(|_| CoreDomainRepositoryError::Integrity("growth_roll_consumption_id"))?
            .as_str()
            .to_owned(),
        roll_kind: "GROWTH_PERCENTILE",
    }];
    if let Some(increase_roll_id) = increase_roll_id {
        consumptions.push(GameplayRollConsumption {
            roll_id: EntityId::new(increase_roll_id)
                .map_err(|_| CoreDomainRepositoryError::Integrity("growth_roll_consumption_id"))?
                .as_str()
                .to_owned(),
            roll_kind: "GROWTH_INCREASE_D10",
        });
    }
    validate_gameplay_roll_consumptions(consumptions, "growth_roll_consumption_reuse")
}

fn fork_child_id(
    fork_id: &str,
    kind: &str,
    source_id: &str,
) -> Result<String, CoreDomainRepositoryError> {
    let digest = format!(
        "{:x}",
        Sha256::digest(format!("{fork_id}:{kind}:{source_id}").as_bytes())
    );
    let value = format!("{kind}_{}", &digest[..32]);
    EntityId::new(&value)
        .map(|id| id.as_str().to_owned())
        .map_err(|_| CoreDomainRepositoryError::InvalidInput("fork_materialized_id"))
}

fn fork_materialized_growth_awards(
    ending: &ForkSnapshotConclusion,
    character_identity_ids: &BTreeMap<String, String>,
) -> Result<Vec<Value>, CoreDomainRepositoryError> {
    let authorized_skills = ending
        .growth_awards
        .iter()
        .map(|award| award.skill_name.clone())
        .collect::<BTreeSet<_>>();
    if authorized_skills.len() != ending.growth_awards.len()
        || ending.growth_awards.iter().any(|award| {
            award.skill_name.trim().is_empty()
                || award.skill_name != award.skill_name.trim()
                || award.skill_name.len() > 128
                || award.reason.trim().is_empty()
        })
    {
        return Err(CoreDomainRepositoryError::Integrity(
            "fork_conclusion_snapshot_shape",
        ));
    }

    let mut consumed_by_skill = BTreeMap::<String, BTreeSet<String>>::new();
    for award in &ending.growth_awards {
        for character_id in &award.consumed_by_character_ids {
            if character_id.trim().is_empty()
                || !consumed_by_skill
                    .entry(award.skill_name.clone())
                    .or_default()
                    .insert(character_id.clone())
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "fork_conclusion_snapshot_shape",
                ));
            }
        }
    }
    for consumed in &ending.consumed_growth_awards {
        if consumed.character_id.trim().is_empty()
            || !authorized_skills.contains(&consumed.skill_name)
            || !consumed_by_skill
                .entry(consumed.skill_name.clone())
                .or_default()
                .insert(consumed.character_id.clone())
        {
            return Err(CoreDomainRepositoryError::Integrity(
                "fork_conclusion_snapshot_shape",
            ));
        }
    }

    Ok(ending
        .growth_awards
        .iter()
        .map(|award| {
            let consumed_by_character_ids = consumed_by_skill
                .get(&award.skill_name)
                .into_iter()
                .flatten()
                .filter_map(|source_character_id| {
                    character_identity_ids.get(source_character_id).cloned()
                })
                .collect::<Vec<_>>();
            serde_json::json!({
                "skill_name": award.skill_name.clone(),
                "reason": award.reason.clone(),
                "consumed_by_character_ids": consumed_by_character_ids
            })
        })
        .collect())
}

fn fork_snapshot_reference_json(snapshot_hash: &str) -> Result<String, CoreDomainRepositoryError> {
    serde_json::to_string(&serde_json::json!({
        "schema_version": 1,
        "kind": "CONTENT_ADDRESSED_FORK_SNAPSHOT",
        "content_address": snapshot_hash,
        "representation": "CAMPAIGN_FORK_MATERIALIZED_ROWS_V1",
        "excluded_private_scopes": [
            CopyScope::KeeperNotes,
            CopyScope::HiddenClues,
            CopyScope::PrivateMessages,
            CopyScope::AiInternalMemory
        ]
    }))
    .map_err(|_| CoreDomainRepositoryError::Serialization)
}
