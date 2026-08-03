async fn apply_session_started_replay_event(
    transaction: &mut Transaction<'_, Postgres>,
    replay: &CanonicalReplayEvent,
    event: CoreDomainEvent,
) -> Result<(), CoreDomainRepositoryError> {
    let CoreDomainEvent::SessionStarted {
        session_id,
        campaign_id,
        room_id,
        scenario_id,
        scene_id,
        scene_key,
        scene_name,
        started_at_unix_ms,
        ..
    } = event
    else {
        return Err(CoreDomainRepositoryError::Integrity(
            "session_replay_event_mismatch",
        ));
    };
            if campaign_id != replay.campaign_id {
                return Err(CoreDomainRepositoryError::Integrity(
                    "session_replay_campaign_mismatch",
                ));
            }
            let started_at = timestamp_from_unix_ms(started_at_unix_ms, "session.started_at")?;
            sqlx::query(
                r#"
                INSERT INTO core_domain.sessions (
                    session_id, campaign_id, room_id, scenario_id, state,
                    active_scene_id, started_at, ended_at, version,
                    visibility_label, visibility_subject,
                    provenance_kind, provenance_reference, provenance_recorded_by,
                    last_event_sequence
                ) VALUES (
                    $1, $2, $3, $4, 'ACTIVE', $5, $6, NULL, 1,
                    $7, $8, $9, $10, $11, $12
                )
                ON CONFLICT (session_id) DO NOTHING
                "#,
            )
            .bind(&session_id)
            .bind(&campaign_id)
            .bind(&room_id)
            .bind(&scenario_id)
            .bind(&scene_id)
            .bind(started_at)
            .bind(&replay.visibility_label)
            .bind(&replay.visibility_subject)
            .bind(&replay.provenance_kind)
            .bind(&replay.provenance_reference)
            .bind(&replay.provenance_recorded_by)
            .bind(replay.sequence)
            .execute(&mut **transaction)
            .await
            .map_err(database_error("replay_session_start"))?;
            let persisted_session = sqlx::query(
                r#"
                SELECT campaign_id, room_id, scenario_id, started_at
                  FROM core_domain.sessions
                 WHERE session_id = $1
                "#,
            )
            .bind(&session_id)
            .fetch_one(&mut **transaction)
            .await
            .map_err(database_error("verify_replayed_session_start"))?;
            if persisted_session.get::<String, _>("campaign_id") != campaign_id
                || persisted_session.get::<String, _>("room_id") != room_id
                || persisted_session.get::<String, _>("scenario_id") != scenario_id
                || persisted_session.get::<DateTime<Utc>, _>("started_at") != started_at
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "session_replay_identity_conflict",
                ));
            }
            sqlx::query(
                r#"
                INSERT INTO public.scenes (
                    scene_id, campaign_id, session_id, scenario_id, room_id,
                    scene_key, name, state, version,
                    visibility_label, visibility_subject,
                    provenance_kind, provenance_reference, provenance_recorded_by,
                    last_event_sequence
                ) VALUES (
                    $1, $2, $3, $4, $5, $6, $7, 'ACTIVE', 1,
                    $8, $9, $10, $11, $12, $13
                )
                ON CONFLICT (scene_id) DO NOTHING
                "#,
            )
            .bind(&scene_id)
            .bind(&campaign_id)
            .bind(&session_id)
            .bind(&scenario_id)
            .bind(&room_id)
            .bind(&scene_key)
            .bind(&scene_name)
            .bind(&replay.visibility_label)
            .bind(&replay.visibility_subject)
            .bind(&replay.provenance_kind)
            .bind(&replay.provenance_reference)
            .bind(&replay.provenance_recorded_by)
            .bind(replay.sequence)
            .execute(&mut **transaction)
            .await
            .map_err(database_error("replay_opening_scene"))?;
            let persisted_scene = sqlx::query(
                r#"
                SELECT campaign_id, session_id, scenario_id, room_id,
                       scene_key, name
                  FROM public.scenes
                 WHERE scene_id = $1
                "#,
            )
            .bind(&scene_id)
            .fetch_one(&mut **transaction)
            .await
            .map_err(database_error("verify_replayed_opening_scene"))?;
            if persisted_scene.get::<String, _>("campaign_id") != campaign_id
                || persisted_scene.get::<String, _>("session_id") != session_id
                || persisted_scene.get::<String, _>("scenario_id") != scenario_id
                || persisted_scene.get::<String, _>("room_id") != room_id
                || persisted_scene.get::<String, _>("scene_key") != scene_key
                || persisted_scene.get::<String, _>("name") != scene_name
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "scene_replay_identity_conflict",
                ));
            }
    Ok(())
}
