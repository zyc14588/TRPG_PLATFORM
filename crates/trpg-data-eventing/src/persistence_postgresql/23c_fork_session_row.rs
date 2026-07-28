async fn apply_fork_session_row(
    transaction: &mut Transaction<'_, Postgres>,
    replay: &CanonicalReplayEvent,
    _fork_id: &String,
    child_campaign_id: &String,
    row: CampaignForkMaterializedRow,
) -> Result<(), CoreDomainRepositoryError> {
    let CampaignForkMaterializedRow::Session {
        session_id,
        room_id,
        scenario_id,
        state,
        active_scene_id,
        started_at_unix_ms,
        ended_at_unix_ms,
        visibility_label,
        visibility_subject,
    } = row else { unreachable!("expected fork Session row"); };
    if state != "ENDED" || ended_at_unix_ms < started_at_unix_ms {
        return Err(CoreDomainRepositoryError::Integrity("fork_session_state"));
    }
    let started_at =
        timestamp_from_unix_ms(started_at_unix_ms, "fork_session.started_at")?;
    let ended_at =
        timestamp_from_unix_ms(ended_at_unix_ms, "fork_session.ended_at")?;
    sqlx::query(
        r#"
        INSERT INTO core_domain.sessions (
            session_id, campaign_id, room_id, scenario_id, state,
            active_scene_id, started_at, ended_at, version,
            visibility_label, visibility_subject,
            provenance_kind, provenance_reference,
            provenance_recorded_by, last_event_sequence
        ) VALUES (
            $1, $2, $3, $4, 'ENDED', $5, $6, $7, 1,
            $8, $9, $10, $11, $12, $13
        )
        ON CONFLICT (session_id) DO NOTHING
        "#,
    )
    .bind(&session_id)
    .bind(child_campaign_id)
    .bind(&room_id)
    .bind(&scenario_id)
    .bind(&active_scene_id)
    .bind(started_at)
    .bind(ended_at)
    .bind(&visibility_label)
    .bind(&visibility_subject)
    .bind(&replay.provenance_kind)
    .bind(&replay.provenance_reference)
    .bind(&replay.provenance_recorded_by)
    .bind(replay.sequence)
    .execute(&mut **transaction)
    .await
    .map_err(database_error("replay_fork_session"))?;
    let matches: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM core_domain.sessions
             WHERE session_id = $1 AND campaign_id = $2
               AND room_id = $3 AND scenario_id = $4
               AND state = 'ENDED'
               AND active_scene_id IS NOT DISTINCT FROM $5
               AND started_at = $6 AND ended_at = $7
               AND visibility_label::TEXT = $8
               AND visibility_subject = $9
               AND last_event_sequence = $10
        )
        "#,
    )
    .bind(&session_id)
    .bind(child_campaign_id)
    .bind(&room_id)
    .bind(&scenario_id)
    .bind(&active_scene_id)
    .bind(started_at)
    .bind(ended_at)
    .bind(&visibility_label)
    .bind(&visibility_subject)
    .bind(replay.sequence)
    .fetch_one(&mut **transaction)
    .await
    .map_err(database_error("verify_replayed_fork_session"))?;
    if !matches {
        return Err(CoreDomainRepositoryError::Integrity(
            "fork_session_identity_conflict",
        ));
    }
    Ok(())
}
