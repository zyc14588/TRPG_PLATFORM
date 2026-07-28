async fn apply_fork_chase_row(
    transaction: &mut Transaction<'_, Postgres>,
    replay: &CanonicalReplayEvent,
    _fork_id: &String,
    child_campaign_id: &String,
    row: CampaignForkMaterializedRow,
) -> Result<(), CoreDomainRepositoryError> {
    let CampaignForkMaterializedRow::Chase {
        chase_id,
        session_id,
        status,
        range_band,
        segment,
        state_json,
        visibility_label,
        visibility_subject,
    } = row else { unreachable!("expected fork Chase row"); };
    let inspected = inspect_chase_state(&state_json).map_err(|_| {
        CoreDomainRepositoryError::Integrity("fork_chase_state_json")
    })?;
    if inspected.chase_id() != chase_id
        || inspected.status() != status
        || u8::try_from(inspected.range()).ok() != Some(range_band)
        || u64::from(inspected.segment()) != segment
        || inspected.version() != 1
    {
        return Err(CoreDomainRepositoryError::Integrity(
            "fork_chase_state_shape",
        ));
    }
    let range_band = i16::from(range_band);
    let segment = i64::try_from(segment).map_err(|_| {
        CoreDomainRepositoryError::Integrity("fork_chase_segment")
    })?;
    sqlx::query(
        r#"
        INSERT INTO public.chase_states (
            chase_id, campaign_id, session_id, status,
            range_band, segment, state_json, version,
            visibility_label, visibility_subject,
            provenance_kind, provenance_reference,
            provenance_recorded_by, last_event_sequence
        ) VALUES (
            $1, $2, $3, $4, $5, $6, $7::JSONB, 1,
            $8, $9, $10, $11, $12, $13
        )
        ON CONFLICT (chase_id) DO NOTHING
        "#,
    )
    .bind(&chase_id)
    .bind(child_campaign_id)
    .bind(&session_id)
    .bind(&status)
    .bind(range_band)
    .bind(segment)
    .bind(&state_json)
    .bind(&visibility_label)
    .bind(&visibility_subject)
    .bind(&replay.provenance_kind)
    .bind(&replay.provenance_reference)
    .bind(&replay.provenance_recorded_by)
    .bind(replay.sequence)
    .execute(&mut **transaction)
    .await
    .map_err(database_error("replay_fork_chase"))?;
    let matches: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM public.chase_states
             WHERE chase_id = $1
               AND campaign_id = $2
               AND session_id = $3
               AND status = $4
               AND range_band = $5
               AND segment = $6
               AND state_json = $7::JSONB
               AND version = 1
               AND visibility_label::TEXT = $8
               AND visibility_subject = $9
               AND last_event_sequence = $10
        )
        "#,
    )
    .bind(&chase_id)
    .bind(child_campaign_id)
    .bind(&session_id)
    .bind(&status)
    .bind(range_band)
    .bind(segment)
    .bind(&state_json)
    .bind(&visibility_label)
    .bind(&visibility_subject)
    .bind(replay.sequence)
    .fetch_one(&mut **transaction)
    .await
    .map_err(database_error("verify_replayed_fork_chase"))?;
    if !matches {
        return Err(CoreDomainRepositoryError::Integrity(
            "fork_chase_identity_conflict",
        ));
    }
    Ok(())
}
