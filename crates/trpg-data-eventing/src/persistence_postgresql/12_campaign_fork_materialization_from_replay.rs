
fn campaign_fork_materialization_from_replay(
    replay_events: &[CanonicalReplayEvent],
    request: &RecordCampaignForkRequest,
) -> Result<CampaignForkMaterialization, CoreDomainRepositoryError> {
    let fork_events = replay_events
        .iter()
        .filter(|replay| {
            replay.stream_id == request.fork_id
                && matches!(
                    replay.event_type.as_str(),
                    "CampaignForkRecorded"
                        | "CampaignForkMaterializationRecorded"
                        | "CampaignForkMaterialized"
                )
        })
        .collect::<Vec<_>>();
    if fork_events.len() < 3 {
        return Err(CoreDomainRepositoryError::Integrity(
            "campaign_fork_canonical_materialization_missing",
        ));
    }
    let first_replay = fork_events[0];
    for (index, replay) in fork_events.iter().enumerate() {
        let expected_stream_version = i64::try_from(index + 1)
            .map_err(|_| CoreDomainRepositoryError::Integrity("fork_stream_version"))?;
        if replay.campaign_id != request.child_campaign_id
            || replay.resource_type != "campaign_fork"
            || replay.resource_id != request.fork_id
            || replay.expected_version != 0
            || replay.stream_version != expected_stream_version
            || replay.command_id != first_replay.command_id
            || replay.request_hash != first_replay.request_hash
            || replay.request_hash_source != "formal_commit"
            || replay.integrity_status != "verified_hmac"
        {
            return Err(CoreDomainRepositoryError::Integrity(
                "campaign_fork_canonical_event_envelope",
            ));
        }
    }

    let recorded: CoreDomainEvent = serde_json::from_value(first_replay.payload.clone())
        .map_err(|_| CoreDomainRepositoryError::Integrity("campaign_fork_lineage_payload"))?;
    recorded.validate_schema_version()?;
    let CoreDomainEvent::CampaignForkRecorded {
        fork_id,
        parent_campaign_id,
        child_campaign_id,
        source_session_id,
        snapshot_hash,
        child_snapshot_hash,
        copy_scopes,
        canonical_snapshot_json,
        reason,
        ..
    } = recorded
    else {
        return Err(CoreDomainRepositoryError::Integrity(
            "campaign_fork_canonical_event_order",
        ));
    };
    if fork_id != request.fork_id
        || parent_campaign_id != request.parent_campaign_id
        || child_campaign_id != request.child_campaign_id
        || source_session_id != request.source_session_id
        || snapshot_hash != request.snapshot_hash
        || copy_scopes != request.copy_scopes
        || reason != request.reason
        || canonical_snapshot_json != fork_snapshot_reference_json(&request.snapshot_hash)?
    {
        return Err(CoreDomainRepositoryError::Integrity(
            "campaign_fork_child_lineage_conflict",
        ));
    }

    let manifest: CoreDomainEvent = serde_json::from_value(fork_events[1].payload.clone())
        .map_err(|_| CoreDomainRepositoryError::Integrity("fork_manifest_replay_payload"))?;
    manifest.validate_schema_version()?;
    let CoreDomainEvent::CampaignForkMaterializationRecorded {
        fork_id: manifest_fork_id,
        child_campaign_id: manifest_child_campaign_id,
        child_session_id,
        child_scenario_id,
        child_snapshot_hash: manifest_child_snapshot_hash,
        child_state_json,
        materialized_row_count,
        batch_count,
        ..
    } = manifest
    else {
        return Err(CoreDomainRepositoryError::Integrity(
            "campaign_fork_canonical_event_order",
        ));
    };
    let batch_count_usize = usize::try_from(batch_count)
        .map_err(|_| CoreDomainRepositoryError::Integrity("fork_batch_count"))?;
    let materialized_row_count_usize = usize::try_from(materialized_row_count)
        .map_err(|_| CoreDomainRepositoryError::Integrity("fork_row_count"))?;
    if manifest_fork_id != request.fork_id
        || manifest_child_campaign_id != request.child_campaign_id
        || manifest_child_snapshot_hash != child_snapshot_hash
        || child_session_id.trim().is_empty()
        || child_scenario_id.trim().is_empty()
        || child_state_json.trim().is_empty()
        || batch_count_usize == 0
        || fork_events.len() != batch_count_usize + 2
    {
        return Err(CoreDomainRepositoryError::Integrity(
            "campaign_fork_canonical_manifest_mismatch",
        ));
    }

    let mut rows = Vec::with_capacity(materialized_row_count_usize);
    let mut batches = Vec::with_capacity(batch_count_usize);
    for (index, replay) in fork_events.iter().skip(2).enumerate() {
        let event: CoreDomainEvent = serde_json::from_value(replay.payload.clone())
            .map_err(|_| CoreDomainRepositoryError::Integrity("fork_batch_replay_payload"))?;
        event.validate_schema_version()?;
        let CoreDomainEvent::CampaignForkMaterialized {
            fork_id: batch_fork_id,
            child_campaign_id: batch_child_campaign_id,
            batch_index,
            batch_count: event_batch_count,
            rows: batch_rows,
            ..
        } = event
        else {
            return Err(CoreDomainRepositoryError::Integrity(
                "campaign_fork_canonical_event_order",
            ));
        };
        let expected_batch_index = u64::try_from(index + 1)
            .map_err(|_| CoreDomainRepositoryError::Integrity("fork_batch_index"))?;
        if batch_fork_id != request.fork_id
            || batch_child_campaign_id != request.child_campaign_id
            || batch_index != expected_batch_index
            || event_batch_count != batch_count
            || batch_rows.is_empty()
            || batch_rows
                .iter()
                .map(CampaignForkMaterializedRow::projection_target_count)
                .sum::<usize>()
                > 32
        {
            return Err(CoreDomainRepositoryError::Integrity(
                "campaign_fork_canonical_batch_mismatch",
            ));
        }
        let data_subject_id = fork_row_data_subject(&batch_rows[0]);
        if batch_rows.iter().any(|row| {
            let (label, subject) = fork_row_visibility(row);
            label != replay.visibility_label
                || subject != replay.visibility_subject
                || fork_row_data_subject(row) != data_subject_id
        }) {
            return Err(CoreDomainRepositoryError::Integrity(
                "campaign_fork_canonical_batch_visibility",
            ));
        }
        rows.extend(batch_rows.iter().cloned());
        batches.push(CampaignForkMaterializationBatch {
            rows: batch_rows,
            visibility_label: replay.visibility_label.clone(),
            visibility_subject: replay.visibility_subject.clone(),
            data_subject_id,
        });
    }
    if rows.len() != materialized_row_count_usize {
        return Err(CoreDomainRepositoryError::Integrity(
            "campaign_fork_canonical_row_count",
        ));
    }

    Ok(CampaignForkMaterialization {
        child_session_id,
        child_scenario_id,
        child_state_json,
        child_snapshot_hash,
        rows,
        batches,
    })
}

fn fork_character_visibility_is_copyable(
    visibility_label: &str,
    visibility_subject: &str,
    owner_user_id: &str,
) -> bool {
    match visibility_label {
        "public" | "party_visible" => visibility_subject == "not_applicable",
        "private_to_player" | "investigator_private" => visibility_subject == owner_user_id,
        _ => false,
    }
}

fn derive_fork_character_visibility(
    character: &ForkSnapshotCharacter,
    sheet: &ForkSnapshotSheet,
) -> Result<(String, String), CoreDomainRepositoryError> {
    if !fork_character_visibility_is_copyable(
        &character.visibility_label,
        &character.visibility_subject,
        &character.owner_user_id,
    ) || !fork_character_visibility_is_copyable(
        &sheet.visibility_label,
        &sheet.visibility_subject,
        &character.owner_user_id,
    ) {
        return Err(CoreDomainRepositoryError::Integrity(
            "fork_character_visibility",
        ));
    }
    // Character and current Sheet share one canonical materialization event.
    // If their source labels differ, derive the least-visible envelope so the
    // fork can never widen either row's audience.
    if matches!(
        sheet.visibility_label.as_str(),
        "private_to_player" | "investigator_private"
    ) {
        return Ok((
            sheet.visibility_label.clone(),
            character.owner_user_id.clone(),
        ));
    }
    if matches!(
        character.visibility_label.as_str(),
        "private_to_player" | "investigator_private"
    ) {
        return Ok((
            character.visibility_label.clone(),
            character.owner_user_id.clone(),
        ));
    }
    if character.visibility_label == "party_visible" || sheet.visibility_label == "party_visible" {
        Ok(("party_visible".to_owned(), "not_applicable".to_owned()))
    } else {
        Ok(("public".to_owned(), "not_applicable".to_owned()))
    }
}

fn replay_event_field<'a>(event: &'a CanonicalReplayEvent, field: &str) -> Option<&'a str> {
    event
        .payload
        .get(field)
        .and_then(Value::as_str)
        .or_else(|| {
            event
                .payload
                .get("data")
                .and_then(Value::as_object)
                .and_then(|data| data.get(field))
                .and_then(Value::as_str)
        })
}

fn fork_source_session_event_sequences(
    replay_events: &[CanonicalReplayEvent],
    campaign_id: &str,
    source_session_id: &str,
    cutoff_event_sequence: i64,
) -> Result<BTreeSet<i64>, CoreDomainRepositoryError> {
    let session_started = replay_events
        .iter()
        .filter(|event| {
            event.event_type == "SessionStarted"
                && replay_event_field(event, "session_id") == Some(source_session_id)
        })
        .collect::<Vec<_>>();
    if session_started.len() != 1
        || session_started[0].campaign_id != campaign_id
        || session_started[0].sequence <= 0
        || session_started[0].sequence > cutoff_event_sequence
        || session_started[0].integrity_status != "verified_hmac"
        || session_started[0].request_hash_source != "formal_commit"
        || session_started[0].event_integrity_hash.is_none()
    {
        return Err(CoreDomainRepositoryError::Integrity(
            "fork_source_session_start",
        ));
    }
    let source_start_sequence = session_started[0].sequence;
    let mut source_scene_ids = BTreeSet::new();
    for event in replay_events.iter().filter(|event| {
        event.sequence <= cutoff_event_sequence
            && replay_event_field(event, "session_id") == Some(source_session_id)
    }) {
        for key in [
            "scene_id",
            "previous_scene_id",
            "next_scene_id",
            "active_scene_id",
        ] {
            if let Some(scene_id) = replay_event_field(event, key) {
                source_scene_ids.insert(scene_id.to_owned());
            }
        }
    }
    if source_scene_ids.is_empty() {
        return Err(CoreDomainRepositoryError::Integrity(
            "fork_source_session_scenes",
        ));
    }
    let source_action_ids = replay_events
        .iter()
        .filter(|event| {
            event.sequence <= cutoff_event_sequence
                && event.event_type == "PlayerActionSubmitted"
                && replay_event_field(event, "scene_id")
                    .is_some_and(|scene_id| source_scene_ids.contains(scene_id))
        })
        .map(|event| {
            replay_event_field(event, "action_id")
                .map(str::to_owned)
                .ok_or(CoreDomainRepositoryError::Integrity(
                    "fork_source_session_action",
                ))
        })
        .collect::<Result<BTreeSet<_>, _>>()?;

    Ok(replay_events
        .iter()
        .filter(|event| {
            event.sequence < source_start_sequence
                || event.sequence <= cutoff_event_sequence
                    && (replay_event_field(event, "session_id") == Some(source_session_id)
                        || replay_event_field(event, "action_id")
                            .is_some_and(|action_id| source_action_ids.contains(action_id))
                        || matches!(
                            event.event_type.as_str(),
                            "CharacterCreated"
                                | "CharacterSubmitted"
                                | "CharacterInitialVersionApproved"
                        ))
        })
        .map(|event| event.sequence)
        .collect())
}
