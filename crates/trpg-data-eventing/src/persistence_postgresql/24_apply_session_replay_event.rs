
async fn apply_session_replay_event(
    transaction: &mut Transaction<'_, Postgres>,
    replay: &CanonicalReplayEvent,
) -> Result<(), CoreDomainRepositoryError> {
    let event: CoreDomainEvent = serde_json::from_value(replay.payload.clone())
        .map_err(|_| CoreDomainRepositoryError::Integrity("session_replay_payload"))?;
    event.validate_schema_version()?;
    match event {
        event @ CoreDomainEvent::SessionStarted { .. } => {
            apply_session_started_replay_event(transaction, replay, event).await?;
        }
        CoreDomainEvent::CharacterJoinedSession {
            join_id,
            campaign_id,
            session_id,
            character_id,
            owner_user_id,
            joined_at_unix_ms,
            ..
        } => {
            if campaign_id != replay.campaign_id {
                return Err(CoreDomainRepositoryError::Integrity(
                    "session_character_replay_campaign",
                ));
            }
            let joined_at =
                timestamp_from_unix_ms(joined_at_unix_ms, "character_session.joined_at")?;
            sqlx::query(
                r#"
                INSERT INTO core_domain.session_characters (
                    join_id, campaign_id, session_id, character_id,
                    owner_user_id, joined_by, joined_at, version,
                    visibility_label, visibility_subject,
                    provenance_kind, provenance_reference, provenance_recorded_by,
                    last_event_sequence
                ) VALUES (
                    $1, $2, $3, $4, $5, $5, $6, 1,
                    $7, $8, $9, $10, $11, $12
                )
                ON CONFLICT (join_id) DO NOTHING
                "#,
            )
            .bind(&join_id)
            .bind(&campaign_id)
            .bind(&session_id)
            .bind(&character_id)
            .bind(&owner_user_id)
            .bind(joined_at)
            .bind(&replay.visibility_label)
            .bind(&replay.visibility_subject)
            .bind(&replay.provenance_kind)
            .bind(&replay.provenance_reference)
            .bind(&replay.provenance_recorded_by)
            .bind(replay.sequence)
            .execute(&mut **transaction)
            .await
            .map_err(database_error("replay_character_session_join"))?;
            let persisted = sqlx::query(
                r#"
                SELECT campaign_id, session_id, character_id, owner_user_id,
                       joined_at, last_event_sequence
                  FROM core_domain.session_characters
                 WHERE join_id = $1
                "#,
            )
            .bind(&join_id)
            .fetch_one(&mut **transaction)
            .await
            .map_err(database_error("verify_replayed_character_session_join"))?;
            if persisted.get::<String, _>("campaign_id") != campaign_id
                || persisted.get::<String, _>("session_id") != session_id
                || persisted.get::<String, _>("character_id") != character_id
                || persisted.get::<String, _>("owner_user_id") != owner_user_id
                || persisted.get::<DateTime<Utc>, _>("joined_at") != joined_at
                || persisted.get::<i64, _>("last_event_sequence") != replay.sequence
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "session_character_replay_conflict",
                ));
            }
        }
        CoreDomainEvent::SessionStateChanged {
            session_id,
            from,
            to,
            changed_at_unix_ms,
            ..
        } => {
            let changed_at = timestamp_from_unix_ms(changed_at_unix_ms, "session.changed_at")?;
            if to == SessionState::Ended {
                sqlx::query(
                    r#"
                    UPDATE public.scenes
                       SET state = 'CLOSED',
                           version = version + 1,
                           visibility_label = $1,
                           visibility_subject = $2,
                           provenance_kind = $3,
                           provenance_reference = $4,
                           provenance_recorded_by = $5,
                           last_event_sequence = $6
                     WHERE scene_id = (
                         SELECT active_scene_id
                           FROM core_domain.sessions
                          WHERE session_id = $7
                     )
                       AND state = 'ACTIVE'
                       AND last_event_sequence < $6
                    "#,
                )
                .bind(&replay.visibility_label)
                .bind(&replay.visibility_subject)
                .bind(&replay.provenance_kind)
                .bind(&replay.provenance_reference)
                .bind(&replay.provenance_recorded_by)
                .bind(replay.sequence)
                .bind(&session_id)
                .execute(&mut **transaction)
                .await
                .map_err(database_error("replay_close_ending_scene"))?;
            }
            let result = sqlx::query(
                r#"
                UPDATE core_domain.sessions
                   SET state = $1,
                       ended_at = CASE WHEN $1 = 'ENDED' THEN $2 ELSE NULL END,
                       version = version + 1,
                       visibility_label = $3,
                       visibility_subject = $4,
                       provenance_kind = $5,
                       provenance_reference = $6,
                       provenance_recorded_by = $7,
                       last_event_sequence = $8
                 WHERE session_id = $9
                   AND state = $10
                   AND last_event_sequence < $8
                "#,
            )
            .bind(to.as_str())
            .bind(changed_at)
            .bind(&replay.visibility_label)
            .bind(&replay.visibility_subject)
            .bind(&replay.provenance_kind)
            .bind(&replay.provenance_reference)
            .bind(&replay.provenance_recorded_by)
            .bind(replay.sequence)
            .bind(&session_id)
            .bind(from.as_str())
            .execute(&mut **transaction)
            .await
            .map_err(database_error("replay_session_transition"))?;
            if result.rows_affected() == 0 {
                let existing = sqlx::query(
                    "SELECT state, last_event_sequence \
                     FROM core_domain.sessions WHERE session_id = $1",
                )
                .bind(&session_id)
                .fetch_one(&mut **transaction)
                .await
                .map_err(database_error("verify_replayed_session_transition"))?;
                if existing.get::<i64, _>("last_event_sequence") < replay.sequence {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "session_replay_transition_conflict",
                    ));
                }
            }
        }
        CoreDomainEvent::SceneSwitched {
            session_id,
            previous_scene_id,
            next_scene_id,
            next_scene_key,
            next_scene_name,
            ..
        } => {
            let session = sqlx::query(
                r#"
                SELECT campaign_id, room_id, scenario_id, active_scene_id,
                       last_event_sequence
                  FROM core_domain.sessions
                 WHERE session_id = $1
                "#,
            )
            .bind(&session_id)
            .fetch_optional(&mut **transaction)
            .await
            .map_err(database_error("replay_load_scene_session"))?
            .ok_or(CoreDomainRepositoryError::Integrity(
                "session_replay_missing_session",
            ))?;
            if session.get::<String, _>("campaign_id") != replay.campaign_id
                || (session.get::<i64, _>("last_event_sequence") < replay.sequence
                    && session
                        .get::<Option<String>, _>("active_scene_id")
                        .as_deref()
                        != Some(previous_scene_id.as_str()))
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "scene_replay_predecessor_mismatch",
                ));
            }
            let closed = sqlx::query(
                r#"
                UPDATE public.scenes
                   SET state = 'CLOSED',
                       version = version + 1,
                       visibility_label = $1,
                       visibility_subject = $2,
                       provenance_kind = $3,
                       provenance_reference = $4,
                       provenance_recorded_by = $5,
                       last_event_sequence = $6
                 WHERE scene_id = $7
                   AND state = 'ACTIVE'
                   AND last_event_sequence < $6
                "#,
            )
            .bind(&replay.visibility_label)
            .bind(&replay.visibility_subject)
            .bind(&replay.provenance_kind)
            .bind(&replay.provenance_reference)
            .bind(&replay.provenance_recorded_by)
            .bind(replay.sequence)
            .bind(&previous_scene_id)
            .execute(&mut **transaction)
            .await
            .map_err(database_error("replay_close_previous_scene"))?;
            if closed.rows_affected() == 0 {
                let existing_sequence: Option<i64> = sqlx::query_scalar(
                    "SELECT last_event_sequence FROM public.scenes WHERE scene_id = $1",
                )
                .bind(&previous_scene_id)
                .fetch_optional(&mut **transaction)
                .await
                .map_err(database_error("verify_replayed_previous_scene"))?;
                if existing_sequence.is_none_or(|sequence| sequence < replay.sequence) {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "scene_replay_previous_not_active",
                    ));
                }
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
            .bind(&next_scene_id)
            .bind(&replay.campaign_id)
            .bind(&session_id)
            .bind(session.get::<String, _>("scenario_id"))
            .bind(session.get::<String, _>("room_id"))
            .bind(&next_scene_key)
            .bind(&next_scene_name)
            .bind(&replay.visibility_label)
            .bind(&replay.visibility_subject)
            .bind(&replay.provenance_kind)
            .bind(&replay.provenance_reference)
            .bind(&replay.provenance_recorded_by)
            .bind(replay.sequence)
            .execute(&mut **transaction)
            .await
            .map_err(database_error("replay_next_scene"))?;
            let persisted_next_scene = sqlx::query(
                r#"
                SELECT campaign_id, session_id, scenario_id, room_id,
                       scene_key, name
                  FROM public.scenes
                 WHERE scene_id = $1
                "#,
            )
            .bind(&next_scene_id)
            .fetch_one(&mut **transaction)
            .await
            .map_err(database_error("verify_replayed_next_scene"))?;
            if persisted_next_scene.get::<String, _>("campaign_id") != replay.campaign_id
                || persisted_next_scene.get::<String, _>("session_id") != session_id
                || persisted_next_scene.get::<String, _>("scenario_id")
                    != session.get::<String, _>("scenario_id")
                || persisted_next_scene.get::<String, _>("room_id")
                    != session.get::<String, _>("room_id")
                || persisted_next_scene.get::<String, _>("scene_key") != next_scene_key
                || persisted_next_scene.get::<String, _>("name") != next_scene_name
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "scene_replay_identity_conflict",
                ));
            }
            let advanced = sqlx::query(
                r#"
                UPDATE core_domain.sessions
                   SET active_scene_id = $1,
                       version = version + 1,
                       visibility_label = $2,
                       visibility_subject = $3,
                       provenance_kind = $4,
                       provenance_reference = $5,
                       provenance_recorded_by = $6,
                       last_event_sequence = $7
                 WHERE session_id = $8
                   AND active_scene_id = $9
                   AND last_event_sequence < $7
                "#,
            )
            .bind(&next_scene_id)
            .bind(&replay.visibility_label)
            .bind(&replay.visibility_subject)
            .bind(&replay.provenance_kind)
            .bind(&replay.provenance_reference)
            .bind(&replay.provenance_recorded_by)
            .bind(replay.sequence)
            .bind(&session_id)
            .bind(&previous_scene_id)
            .execute(&mut **transaction)
            .await
            .map_err(database_error("replay_active_scene"))?;
            if advanced.rows_affected() == 0 {
                let existing = sqlx::query(
                    "SELECT active_scene_id, last_event_sequence \
                     FROM core_domain.sessions WHERE session_id = $1",
                )
                .bind(&session_id)
                .fetch_one(&mut **transaction)
                .await
                .map_err(database_error("verify_replayed_active_scene"))?;
                if existing.get::<i64, _>("last_event_sequence") < replay.sequence {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "scene_replay_session_conflict",
                    ));
                }
            }
        }
        _ => {
            return Err(CoreDomainRepositoryError::Integrity(
                "non_session_event_in_session_replay",
            ));
        }
    }
    Ok(())
}

include!("24_apply_session_replay_event/01_apply_session_started.rs");
