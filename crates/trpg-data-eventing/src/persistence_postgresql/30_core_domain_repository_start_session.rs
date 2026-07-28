
impl CoreDomainRepository {

    pub async fn start_session(
        &self,
        metadata: &CoreCommandMetadata,
        request: &StartSessionRequest,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        if metadata.expected_version != 0
            || request.scene_key.trim().is_empty()
            || request.scene_name.trim().is_empty()
        {
            return Err(CoreDomainRepositoryError::InvalidInput("session_start"));
        }
        self.ensure_campaign_admin(&request.campaign_id, &metadata.requesting_actor_id)
            .await?;
        let references_exist: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1
                  FROM public.rooms AS room
                  JOIN public.scenarios AS scenario
                    ON scenario.campaign_id = room.campaign_id
                 WHERE room.room_id = $1
                   AND scenario.scenario_id = $2
                   AND room.campaign_id = $3
            )
            "#,
        )
        .bind(&request.room_id)
        .bind(&request.scenario_id)
        .bind(&request.campaign_id)
        .fetch_one(&self.primary)
        .await
        .map_err(database_error("load_session_references"))?;
        if !references_exist {
            return Err(CoreDomainRepositoryError::NotFound(
                "session_room_or_scenario",
            ));
        }
        let mut session = Session::scheduled(
            &request.session_id,
            &request.campaign_id,
            &request.room_id,
            &request.scenario_id,
        )?;
        session.transition(SessionState::Active)?;
        let started_at = timestamp_from_unix_ms(request.started_at_unix_ms, "session.started_at")?;
        let event = CoreDomainEvent::SessionStarted {
            schema_version: CORE_EVENT_SCHEMA_VERSION,
            session_id: request.session_id.clone(),
            campaign_id: request.campaign_id.clone(),
            room_id: request.room_id.clone(),
            scenario_id: request.scenario_id.clone(),
            scene_id: request.scene_id.clone(),
            scene_key: request.scene_key.clone(),
            scene_name: request.scene_name.clone(),
            started_at_unix_ms: request.started_at_unix_ms,
        };

        let mut transaction = self
            .begin_projection_transaction(&metadata.commit_id, "begin_session_start")
            .await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(format!(
                "core-session:{}:{}",
                request.campaign_id, request.room_id
            ))
            .execute(&mut *transaction)
            .await
            .map_err(database_error("lock_session_room"))?;
        if let Some(existing_sequence) = sqlx::query_scalar::<_, i64>(
            "SELECT last_event_sequence FROM core_domain.sessions WHERE session_id = $1",
        )
        .bind(&request.session_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(database_error("load_existing_session"))?
        {
            if self
                .projection_matches_command(existing_sequence, metadata)
                .await?
            {
                return self
                    .commit_event(
                        metadata,
                        &request.campaign_id,
                        &request.session_id,
                        ("session", "session.start"),
                        &event,
                        vec![
                            projection_target("core_domain.sessions", &request.session_id),
                            projection_target("public.scenes", &request.scene_id),
                        ],
                    )
                    .await;
            }
            return Err(CoreDomainRepositoryError::ConcurrentStart);
        }
        ensure_scenario_scene_key(
            &mut transaction,
            &request.campaign_id,
            &request.scenario_id,
            &request.scene_key,
        )
        .await?;
        let live_exists: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM core_domain.sessions
                 WHERE campaign_id = $1
                   AND room_id = $2
                   AND state IN ('ACTIVE', 'PAUSED')
            )
            "#,
        )
        .bind(&request.campaign_id)
        .bind(&request.room_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(database_error("check_live_session"))?;
        if live_exists {
            return Err(CoreDomainRepositoryError::ConcurrentStart);
        }
        let scene_exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM public.scenes WHERE scene_id = $1)")
                .bind(&request.scene_id)
                .fetch_one(&mut *transaction)
                .await
                .map_err(database_error("check_scene_identity"))?;
        if scene_exists {
            return Err(CoreDomainRepositoryError::Integrity(
                "scene_identity_conflict",
            ));
        }
        let persisted = self
            .commit_event(
                metadata,
                &request.campaign_id,
                &request.session_id,
                ("session", "session.start"),
                &event,
                vec![
                    projection_target("core_domain.sessions", &request.session_id),
                    projection_target("public.scenes", &request.scene_id),
                ],
            )
            .await?;
        sqlx::query("SET CONSTRAINTS ALL DEFERRED")
            .execute(&mut *transaction)
            .await
            .map_err(database_error("defer_session_constraints"))?;
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
            "#,
        )
        .bind(&request.session_id)
        .bind(&request.campaign_id)
        .bind(&request.room_id)
        .bind(&request.scenario_id)
        .bind(&request.scene_id)
        .bind(started_at)
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(persisted.last_event_sequence)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("insert_session"))?;
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
        .bind(&request.scene_id)
        .bind(&request.campaign_id)
        .bind(&request.session_id)
        .bind(&request.scenario_id)
        .bind(&request.room_id)
        .bind(&request.scene_key)
        .bind(&request.scene_name)
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(persisted.last_event_sequence)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("insert_opening_scene"))?;
        transaction
            .commit()
            .await
            .map_err(database_error("commit_session_start"))?;
        Ok(persisted)
    }
}
