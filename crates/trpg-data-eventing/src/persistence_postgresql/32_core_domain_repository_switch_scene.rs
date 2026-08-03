
impl CoreDomainRepository {

    pub async fn switch_scene(
        &self,
        metadata: &CoreCommandMetadata,
        request: &SwitchSceneRequest,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        if request.next_scene_key.trim().is_empty() || request.next_scene_name.trim().is_empty() {
            return Err(CoreDomainRepositoryError::InvalidInput("next_scene"));
        }
        self.ensure_campaign_admin(&request.campaign_id, &metadata.requesting_actor_id)
            .await?;
        let _switched_at =
            timestamp_from_unix_ms(request.switched_at_unix_ms, "scene.switched_at")?;
        let mut transaction = self
            .begin_projection_transaction(&metadata.commit_id, "begin_scene_switch")
            .await?;
        let session_row = sqlx::query(
            r#"
            SELECT campaign_id, room_id, scenario_id, state, active_scene_id,
                   version, last_event_sequence
              FROM core_domain.sessions
             WHERE session_id = $1
             FOR UPDATE
            "#,
        )
        .bind(&request.session_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(database_error("load_session_for_scene_switch"))?
        .ok_or(CoreDomainRepositoryError::NotFound("session"))?;
        if session_row.get::<String, _>("campaign_id") != request.campaign_id {
            return Err(CoreDomainRepositoryError::Forbidden);
        }
        if session_row.get::<String, _>("state") != "ACTIVE" {
            return Err(CoreDomainRepositoryError::Domain(
                CoreEntityError::InvalidTransition {
                    aggregate: "scene",
                    from: "INACTIVE_SESSION",
                    to: "ACTIVE",
                },
            ));
        }
        let current_version: i64 = session_row.get("version");
        let current_active_scene_id = session_row.get::<Option<String>, _>("active_scene_id");
        let current_event_sequence: i64 = session_row.get("last_event_sequence");
        if current_active_scene_id.as_deref() == Some(request.next_scene_id.as_str())
            && self
                .projection_matches_command(current_event_sequence, metadata)
                .await?
        {
            let existing_event = self
                .load_idempotent_core_event(
                    &request.campaign_id,
                    &request.session_id,
                    metadata,
                    "SceneSwitched",
                )
                .await?
                .ok_or(CoreDomainRepositoryError::Integrity(
                    "idempotent_scene_event_missing",
                ))?;
            let previous_scene_id = match &existing_event {
                CoreDomainEvent::SceneSwitched {
                    session_id,
                    previous_scene_id,
                    next_scene_id,
                    next_scene_key,
                    next_scene_name,
                    switched_at_unix_ms,
                    ..
                } if session_id == &request.session_id
                    && next_scene_id == &request.next_scene_id
                    && next_scene_key == &request.next_scene_key
                    && next_scene_name == &request.next_scene_name
                    && *switched_at_unix_ms == request.switched_at_unix_ms =>
                {
                    previous_scene_id.clone()
                }
                _ => {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "idempotent_scene_request_conflict",
                    ))
                }
            };
            return self
                .commit_event(
                    metadata,
                    &request.campaign_id,
                    &request.session_id,
                    ("session", "scene.switch"),
                    &existing_event,
                    vec![
                        projection_target("core_domain.sessions", &request.session_id),
                        projection_target("public.scenes", &previous_scene_id),
                        projection_target("public.scenes", &request.next_scene_id),
                    ],
                )
                .await;
        }
        if metadata.expected_version != current_version {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "session_expected_version",
            ));
        }
        ensure_scenario_scene_key(
            &mut transaction,
            &request.campaign_id,
            &session_row.get::<String, _>("scenario_id"),
            &request.next_scene_key,
        )
        .await?;
        let previous_scene_id = current_active_scene_id
            .ok_or(CoreDomainRepositoryError::Integrity("active_scene_missing"))?;
        let identity_conflict: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM public.scenes
                 WHERE scene_id = $1
                    OR (session_id = $2 AND scene_key = $3)
            )
            "#,
        )
        .bind(&request.next_scene_id)
        .bind(&request.session_id)
        .bind(&request.next_scene_key)
        .fetch_one(&mut *transaction)
        .await
        .map_err(database_error("check_next_scene_identity"))?;
        if identity_conflict {
            return Err(CoreDomainRepositoryError::Integrity(
                "scene_identity_conflict",
            ));
        }
        let event = CoreDomainEvent::SceneSwitched {
            schema_version: CORE_EVENT_SCHEMA_VERSION,
            session_id: request.session_id.clone(),
            previous_scene_id: previous_scene_id.clone(),
            next_scene_id: request.next_scene_id.clone(),
            next_scene_key: request.next_scene_key.clone(),
            next_scene_name: request.next_scene_name.clone(),
            switched_at_unix_ms: request.switched_at_unix_ms,
        };
        let persisted = self
            .commit_event(
                metadata,
                &request.campaign_id,
                &request.session_id,
                ("session", "scene.switch"),
                &event,
                vec![
                    projection_target("core_domain.sessions", &request.session_id),
                    projection_target("public.scenes", &previous_scene_id),
                    projection_target("public.scenes", &request.next_scene_id),
                ],
            )
            .await?;
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
               AND session_id = $8
               AND state = 'ACTIVE'
            "#,
        )
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(persisted.last_event_sequence)
        .bind(&previous_scene_id)
        .bind(&request.session_id)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("close_previous_scene"))?;
        if closed.rows_affected() != 1 {
            return Err(CoreDomainRepositoryError::Integrity(
                "previous_scene_not_active",
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
            "#,
        )
        .bind(&request.next_scene_id)
        .bind(&request.campaign_id)
        .bind(&request.session_id)
        .bind(session_row.get::<String, _>("scenario_id"))
        .bind(session_row.get::<String, _>("room_id"))
        .bind(&request.next_scene_key)
        .bind(&request.next_scene_name)
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(persisted.last_event_sequence)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("insert_next_scene"))?;
        sqlx::query(
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
               AND version = $10
            "#,
        )
        .bind(&request.next_scene_id)
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(persisted.last_event_sequence)
        .bind(&request.session_id)
        .bind(&previous_scene_id)
        .bind(current_version)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("project_active_scene"))?;
        transaction
            .commit()
            .await
            .map_err(database_error("commit_scene_switch"))?;
        Ok(persisted)
    }

    pub async fn rebuild_session_scene_projection(
        &self,
        campaign_id: &str,
    ) -> Result<SessionProjectionRebuildReport, CoreDomainRepositoryError> {
        let replay = self.load_campaign_events(campaign_id).await?;
        let session_events = replay
            .into_iter()
            .filter(|event| {
                matches!(
                    event.event_type.as_str(),
                    "SessionStarted"
                        | "CharacterJoinedSession"
                        | "SessionStateChanged"
                        | "SceneSwitched"
                )
            })
            .collect::<Vec<_>>();
        let last_event_sequence = session_events
            .last()
            .map(|event| event.sequence)
            .unwrap_or(0);
        let mut transaction = self
            .primary
            .begin()
            .await
            .map_err(database_error("begin_session_projection_rebuild"))?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(format!("core-session-rebuild:{campaign_id}"))
            .execute(&mut *transaction)
            .await
            .map_err(database_error("lock_session_projection_rebuild"))?;
        sqlx::query("SET CONSTRAINTS ALL DEFERRED")
            .execute(&mut *transaction)
            .await
            .map_err(database_error("defer_rebuild_constraints"))?;
        for replay_event in &session_events {
            let commit_id: String = sqlx::query_scalar(
                r#"
                SELECT commit_id
                  FROM public.formal_commits
                 WHERE $1 BETWEEN first_event_sequence AND last_event_sequence
                   AND campaign_id = $2
                   AND status = 'committed'
                "#,
            )
            .bind(replay_event.sequence)
            .bind(campaign_id)
            .fetch_one(&mut *transaction)
            .await
            .map_err(database_error("load_rebuild_projection_commit"))?;
            self.set_projection_capability(
                &mut transaction,
                &commit_id,
                "set_rebuild_projection_capability",
            )
            .await?;
            apply_session_replay_event(&mut transaction, replay_event).await?;
        }
        let restored_sessions: i64 =
            sqlx::query_scalar("SELECT count(*) FROM core_domain.sessions WHERE campaign_id = $1")
                .bind(campaign_id)
                .fetch_one(&mut *transaction)
                .await
                .map_err(database_error("count_rebuilt_sessions"))?;
        let restored_scenes: i64 =
            sqlx::query_scalar("SELECT count(*) FROM public.scenes WHERE campaign_id = $1")
                .bind(campaign_id)
                .fetch_one(&mut *transaction)
                .await
                .map_err(database_error("count_rebuilt_scenes"))?;
        transaction
            .commit()
            .await
            .map_err(database_error("commit_session_projection_rebuild"))?;
        Ok(SessionProjectionRebuildReport {
            campaign_id: campaign_id.to_owned(),
            replayed_events: session_events.len(),
            restored_sessions,
            restored_scenes,
            last_event_sequence,
        })
    }
}
