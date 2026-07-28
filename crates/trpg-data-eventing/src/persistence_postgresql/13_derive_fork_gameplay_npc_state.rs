
fn derive_fork_gameplay_npc_state(state: &Value) -> Result<Vec<Value>, CoreDomainRepositoryError> {
    let character_ids = state
        .get("character_state")
        .and_then(Value::as_array)
        .ok_or(CoreDomainRepositoryError::Integrity(
            "fork_character_snapshot_shape",
        ))?
        .iter()
        .map(|character| {
            character
                .get("character_id")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or(CoreDomainRepositoryError::Integrity(
                    "fork_character_snapshot_shape",
                ))
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    let existing_npcs = state.get("npc_state").and_then(Value::as_array).ok_or(
        CoreDomainRepositoryError::Integrity("fork_npc_snapshot_shape"),
    )?;
    let mut npc_state = BTreeMap::<String, Value>::new();
    for npc in existing_npcs {
        let npc_id = npc.get("npc_id").and_then(Value::as_str).ok_or(
            CoreDomainRepositoryError::Integrity("fork_npc_snapshot_shape"),
        )?;
        if character_ids.contains(npc_id)
            || npc_state.insert(npc_id.to_owned(), npc.clone()).is_some()
        {
            return Err(CoreDomainRepositoryError::Integrity(
                "fork_npc_snapshot_identity",
            ));
        }
    }
    for scope in ["combat_state", "chase_state"] {
        let aggregates = state.get(scope).and_then(Value::as_array).ok_or(
            CoreDomainRepositoryError::Integrity("fork_gameplay_snapshot_shape"),
        )?;
        for aggregate in aggregates {
            let visibility_label = aggregate
                .get("visibility_label")
                .and_then(Value::as_str)
                .filter(|label| matches!(*label, "public" | "party_visible"))
                .ok_or(CoreDomainRepositoryError::Integrity(
                    "fork_gameplay_snapshot_visibility",
                ))?;
            let visibility_subject = aggregate
                .get("visibility_subject")
                .and_then(Value::as_str)
                .filter(|subject| *subject == "not_applicable")
                .ok_or(CoreDomainRepositoryError::Integrity(
                    "fork_gameplay_snapshot_visibility",
                ))?;
            let participants = aggregate
                .pointer("/state/participants")
                .and_then(Value::as_array)
                .ok_or(CoreDomainRepositoryError::Integrity(
                    "fork_gameplay_snapshot_participants",
                ))?;
            for participant in participants {
                let participant_id = participant
                    .get("participant_id")
                    .and_then(Value::as_str)
                    .ok_or(CoreDomainRepositoryError::Integrity(
                        "fork_gameplay_snapshot_participant",
                    ))?;
                EntityId::new(participant_id).map_err(|_| {
                    CoreDomainRepositoryError::Integrity("fork_gameplay_snapshot_participant")
                })?;
                if character_ids.contains(participant_id) {
                    continue;
                }
                npc_state
                    .entry(participant_id.to_owned())
                    .and_modify(|npc| {
                        if visibility_label == "party_visible" {
                            npc["visibility_label"] = Value::String("party_visible".to_owned());
                        }
                    })
                    .or_insert_with(|| {
                        serde_json::json!({
                            "npc_id": participant_id,
                            "state": {
                                "kind": "FORK_GAMEPLAY_PARTICIPANT",
                                "source_participant_id": participant_id
                            },
                            "visibility_label": visibility_label,
                            "visibility_subject": visibility_subject
                        })
                    });
            }
        }
    }
    Ok(npc_state.into_values().collect())
}

fn rewrite_fork_gameplay_identity_references(
    value: &mut Value,
    identity_ids: &BTreeMap<String, String>,
) -> Result<(), CoreDomainRepositoryError> {
    match value {
        Value::Array(values) => {
            for value in values {
                rewrite_fork_gameplay_identity_references(value, identity_ids)?;
            }
        }
        Value::Object(fields) => {
            for (key, value) in fields {
                if matches!(
                    key.as_str(),
                    "participant_id" | "attacker_id" | "target_id" | "healer_id"
                ) {
                    let source_id = value.as_str().ok_or(CoreDomainRepositoryError::Integrity(
                        "fork_gameplay_identity_reference",
                    ))?;
                    *value = Value::String(identity_ids.get(source_id).cloned().ok_or(
                        CoreDomainRepositoryError::Integrity("fork_gameplay_identity_missing"),
                    )?);
                } else if key == "initiative_order" {
                    let order =
                        value
                            .as_array_mut()
                            .ok_or(CoreDomainRepositoryError::Integrity(
                                "fork_gameplay_initiative_order",
                            ))?;
                    for participant_id in order {
                        let source_id =
                            participant_id
                                .as_str()
                                .ok_or(CoreDomainRepositoryError::Integrity(
                                    "fork_gameplay_initiative_order",
                                ))?;
                        *participant_id =
                            Value::String(identity_ids.get(source_id).cloned().ok_or(
                                CoreDomainRepositoryError::Integrity(
                                    "fork_gameplay_identity_missing",
                                ),
                            )?);
                    }
                } else {
                    rewrite_fork_gameplay_identity_references(value, identity_ids)?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}
