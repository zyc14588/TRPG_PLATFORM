
async fn apply_combat_replay_event(
    transaction: &mut Transaction<'_, Postgres>,
    replay: &CanonicalReplayEvent,
    event: &CoreDomainEvent,
) -> Result<(), CoreDomainRepositoryError> {
    let CoreDomainEvent::CombatStateRecorded {
        combat_id,
        campaign_id,
        session_id,
        status,
        round,
        turn_index,
        version,
        state_json,
        character_health_updates,
        ..
    } = event
    else {
        return Err(CoreDomainRepositoryError::Integrity(
            "combat_replay_event_type",
        ));
    };
    if campaign_id != &replay.campaign_id
        || !matches!(status.as_str(), "ONGOING" | "ENDED")
        || *version == 0
        || *round == 0
    {
        return Err(CoreDomainRepositoryError::Integrity(
            "combat_replay_event_shape",
        ));
    }
    let version = i64::try_from(*version)
        .map_err(|_| CoreDomainRepositoryError::Integrity("combat_replay_version"))?;
    let round = i64::try_from(*round)
        .map_err(|_| CoreDomainRepositoryError::Integrity("combat_replay_round"))?;
    let turn_index = i64::try_from(*turn_index)
        .map_err(|_| CoreDomainRepositoryError::Integrity("combat_replay_turn"))?;
    let state_value: Value = serde_json::from_str(state_json)
        .map_err(|_| CoreDomainRepositoryError::Integrity("combat_replay_state"))?;
    let roll_consumptions = combat_gameplay_roll_consumptions(state_json)?;
    if state_value.get("combat_id").and_then(Value::as_str) != Some(combat_id)
        || state_value.get("status").and_then(Value::as_str) != Some(status)
        || state_value.get("round").and_then(Value::as_u64) != u64::try_from(round).ok()
        || state_value
            .get("current_turn_index")
            .and_then(Value::as_u64)
            != u64::try_from(turn_index).ok()
        || state_value.get("version").and_then(Value::as_u64) != u64::try_from(version).ok()
    {
        return Err(CoreDomainRepositoryError::Integrity(
            "combat_replay_state_mismatch",
        ));
    }
    let current: Option<(i64, Value)> =
        sqlx::query_as("SELECT version, state_json FROM public.combat_states WHERE combat_id = $1")
            .bind(combat_id)
            .fetch_optional(&mut **transaction)
            .await
            .map_err(database_error("load_combat_replay_state"))?;
    if current
        .as_ref()
        .is_some_and(|(current, _)| *current > version)
    {
        return Ok(());
    }
    if current
        .as_ref()
        .is_some_and(|(current, _)| *current < version - 1)
    {
        return Err(CoreDomainRepositoryError::Integrity(
            "combat_replay_sequence_gap",
        ));
    }
    let replay_health_changes = if current
        .as_ref()
        .is_some_and(|(current_version, _)| *current_version == version - 1)
    {
        Some(combat_health_changes(
            current.as_ref().map(|(_, value)| value),
            &state_value,
        )?)
    } else {
        None
    };
    Box::pin(apply_combat_health_replay_updates(
        transaction,
        replay,
        combat_id,
        version,
        &state_value,
        replay_health_changes.as_deref(),
        character_health_updates,
    ))
    .await?;
    if current.as_ref().map(|(current, _)| *current) != Some(version) {
        let previous_json = current
            .as_ref()
            .map(|(_, value)| serde_json::to_string(value))
            .transpose()
            .map_err(|_| CoreDomainRepositoryError::Serialization)?;
        validate_combat_state_transition(previous_json.as_deref(), state_json)
            .map_err(|_| CoreDomainRepositoryError::Integrity("combat_replay_transition"))?;
        sqlx::query(
            r#"
            INSERT INTO public.combat_states (
                combat_id, campaign_id, session_id, status, round,
                current_turn_index, state_json, version,
                visibility_label, visibility_subject,
                provenance_kind, provenance_reference, provenance_recorded_by,
                last_event_sequence
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7::JSONB, $8,
                $9, $10, $11, $12, $13, $14
            )
            ON CONFLICT (combat_id) DO UPDATE
               SET status = EXCLUDED.status,
                   round = EXCLUDED.round,
                   current_turn_index = EXCLUDED.current_turn_index,
                   state_json = EXCLUDED.state_json,
                   version = EXCLUDED.version,
                   visibility_label = EXCLUDED.visibility_label,
                   visibility_subject = EXCLUDED.visibility_subject,
                   provenance_kind = EXCLUDED.provenance_kind,
                   provenance_reference = EXCLUDED.provenance_reference,
                   provenance_recorded_by = EXCLUDED.provenance_recorded_by,
                   last_event_sequence = EXCLUDED.last_event_sequence
             WHERE combat_states.campaign_id = EXCLUDED.campaign_id
               AND combat_states.session_id = EXCLUDED.session_id
               AND combat_states.version = EXCLUDED.version - 1
               AND combat_states.status = 'ONGOING'
            "#,
        )
        .bind(combat_id)
        .bind(campaign_id)
        .bind(session_id)
        .bind(status)
        .bind(round)
        .bind(turn_index)
        .bind(state_json)
        .bind(version)
        .bind(&replay.visibility_label)
        .bind(&replay.visibility_subject)
        .bind(&replay.provenance_kind)
        .bind(&replay.provenance_reference)
        .bind(&replay.provenance_recorded_by)
        .bind(replay.sequence)
        .execute(&mut **transaction)
        .await
        .map_err(database_error("replay_combat_state"))?;
    }
    project_gameplay_roll_consumptions(
        transaction,
        &roll_consumptions,
        campaign_id,
        "COMBAT",
        combat_id,
        &replay.visibility_label,
        &replay.visibility_subject,
        &replay.provenance_kind,
        &replay.provenance_reference,
        &replay.provenance_recorded_by,
        replay.sequence,
    )
    .await?;
    let matches: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM public.combat_states
             WHERE combat_id = $1 AND campaign_id = $2 AND session_id = $3
               AND status = $4 AND round = $5 AND current_turn_index = $6
               AND state_json = $7::JSONB AND version = $8
               AND last_event_sequence = $9
        )
        "#,
    )
    .bind(combat_id)
    .bind(campaign_id)
    .bind(session_id)
    .bind(status)
    .bind(round)
    .bind(turn_index)
    .bind(state_json)
    .bind(version)
    .bind(replay.sequence)
    .fetch_one(&mut **transaction)
    .await
    .map_err(database_error("verify_replayed_combat_state"))?;
    if !matches {
        return Err(CoreDomainRepositoryError::Integrity(
            "combat_replay_projection_mismatch",
        ));
    }
    Ok(())
}
