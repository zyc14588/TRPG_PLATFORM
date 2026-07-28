
async fn apply_chase_replay_event(
    transaction: &mut Transaction<'_, Postgres>,
    replay: &CanonicalReplayEvent,
    event: &CoreDomainEvent,
) -> Result<(), CoreDomainRepositoryError> {
    let CoreDomainEvent::ChaseStateRecorded {
        chase_id,
        campaign_id,
        session_id,
        status,
        range_band,
        segment,
        version,
        state_json,
        ..
    } = event
    else {
        return Err(CoreDomainRepositoryError::Integrity(
            "chase_replay_event_type",
        ));
    };
    if campaign_id != &replay.campaign_id
        || !matches!(status.as_str(), "ONGOING" | "ESCAPED" | "CAUGHT")
        || *range_band > 5
        || *segment == 0
        || *version == 0
    {
        return Err(CoreDomainRepositoryError::Integrity(
            "chase_replay_event_shape",
        ));
    }
    let version = i64::try_from(*version)
        .map_err(|_| CoreDomainRepositoryError::Integrity("chase_replay_version"))?;
    let segment = i64::try_from(*segment)
        .map_err(|_| CoreDomainRepositoryError::Integrity("chase_replay_segment"))?;
    let range_band = i16::from(*range_band);
    let state_value: Value = serde_json::from_str(state_json)
        .map_err(|_| CoreDomainRepositoryError::Integrity("chase_replay_state"))?;
    let roll_consumptions = chase_gameplay_roll_consumptions(state_json)?;
    if state_value.get("chase_id").and_then(Value::as_str) != Some(chase_id)
        || state_value.get("status").and_then(Value::as_str) != Some(status)
        || state_value.get("range").and_then(Value::as_i64) != Some(i64::from(range_band))
        || state_value.get("segment").and_then(Value::as_u64) != u64::try_from(segment).ok()
        || state_value.get("version").and_then(Value::as_u64) != u64::try_from(version).ok()
    {
        return Err(CoreDomainRepositoryError::Integrity(
            "chase_replay_state_mismatch",
        ));
    }
    let current: Option<(i64, Value)> =
        sqlx::query_as("SELECT version, state_json FROM public.chase_states WHERE chase_id = $1")
            .bind(chase_id)
            .fetch_optional(&mut **transaction)
            .await
            .map_err(database_error("load_chase_replay_state"))?;
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
            "chase_replay_sequence_gap",
        ));
    }
    if current.as_ref().map(|(current, _)| *current) != Some(version) {
        let previous_json = current
            .as_ref()
            .map(|(_, value)| serde_json::to_string(value))
            .transpose()
            .map_err(|_| CoreDomainRepositoryError::Serialization)?;
        validate_chase_state_transition(previous_json.as_deref(), state_json)
            .map_err(|_| CoreDomainRepositoryError::Integrity("chase_replay_transition"))?;
        sqlx::query(
            r#"
            INSERT INTO public.chase_states (
                chase_id, campaign_id, session_id, status, range_band,
                segment, state_json, version,
                visibility_label, visibility_subject,
                provenance_kind, provenance_reference, provenance_recorded_by,
                last_event_sequence
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7::JSONB, $8,
                $9, $10, $11, $12, $13, $14
            )
            ON CONFLICT (chase_id) DO UPDATE
               SET status = EXCLUDED.status,
                   range_band = EXCLUDED.range_band,
                   segment = EXCLUDED.segment,
                   state_json = EXCLUDED.state_json,
                   version = EXCLUDED.version,
                   visibility_label = EXCLUDED.visibility_label,
                   visibility_subject = EXCLUDED.visibility_subject,
                   provenance_kind = EXCLUDED.provenance_kind,
                   provenance_reference = EXCLUDED.provenance_reference,
                   provenance_recorded_by = EXCLUDED.provenance_recorded_by,
                   last_event_sequence = EXCLUDED.last_event_sequence
             WHERE chase_states.campaign_id = EXCLUDED.campaign_id
               AND chase_states.session_id = EXCLUDED.session_id
               AND chase_states.version = EXCLUDED.version - 1
               AND chase_states.status = 'ONGOING'
            "#,
        )
        .bind(chase_id)
        .bind(campaign_id)
        .bind(session_id)
        .bind(status)
        .bind(range_band)
        .bind(segment)
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
        .map_err(database_error("replay_chase_state"))?;
    }
    project_gameplay_roll_consumptions(
        transaction,
        &roll_consumptions,
        campaign_id,
        "CHASE",
        chase_id,
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
            SELECT 1 FROM public.chase_states
             WHERE chase_id = $1 AND campaign_id = $2 AND session_id = $3
               AND status = $4 AND range_band = $5 AND segment = $6
               AND state_json = $7::JSONB AND version = $8
               AND last_event_sequence = $9
        )
        "#,
    )
    .bind(chase_id)
    .bind(campaign_id)
    .bind(session_id)
    .bind(status)
    .bind(range_band)
    .bind(segment)
    .bind(state_json)
    .bind(version)
    .bind(replay.sequence)
    .fetch_one(&mut **transaction)
    .await
    .map_err(database_error("verify_replayed_chase_state"))?;
    if !matches {
        return Err(CoreDomainRepositoryError::Integrity(
            "chase_replay_projection_mismatch",
        ));
    }
    Ok(())
}
