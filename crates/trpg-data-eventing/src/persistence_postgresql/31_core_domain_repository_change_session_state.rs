
impl CoreDomainRepository {

    pub async fn change_session_state(
        &self,
        metadata: &CoreCommandMetadata,
        campaign_id: &str,
        session_id: &str,
        next_state: SessionState,
        changed_at_unix_ms: u64,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        self.ensure_campaign_admin(campaign_id, &metadata.requesting_actor_id)
            .await?;
        let changed_at = timestamp_from_unix_ms(changed_at_unix_ms, "session.changed_at")?;
        let mut transaction = self
            .begin_projection_transaction(&metadata.commit_id, "begin_session_transition")
            .await?;
        let row = sqlx::query(
            r#"
            SELECT campaign_id, room_id, scenario_id, state, active_scene_id,
                   version, last_event_sequence
              FROM core_domain.sessions
             WHERE session_id = $1
             FOR UPDATE
            "#,
        )
        .bind(session_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(database_error("load_session_for_transition"))?
        .ok_or(CoreDomainRepositoryError::NotFound("session"))?;
        if row.get::<String, _>("campaign_id") != campaign_id {
            return Err(CoreDomainRepositoryError::Forbidden);
        }
        let current_version: i64 = row.get("version");
        let current_state = parse_session_state(&row.get::<String, _>("state"))?;
        let current_event_sequence: i64 = row.get("last_event_sequence");
        if current_state == next_state
            && self
                .projection_matches_command(current_event_sequence, metadata)
                .await?
        {
            let existing_event = self
                .load_idempotent_core_event(
                    campaign_id,
                    session_id,
                    metadata,
                    "SessionStateChanged",
                )
                .await?
                .ok_or(CoreDomainRepositoryError::Integrity(
                    "idempotent_session_event_missing",
                ))?;
            if !matches!(
                &existing_event,
                CoreDomainEvent::SessionStateChanged {
                    session_id: event_session_id,
                    to,
                    changed_at_unix_ms: event_changed_at,
                    ..
                } if event_session_id == session_id
                    && *to == next_state
                    && *event_changed_at == changed_at_unix_ms
            ) {
                return Err(CoreDomainRepositoryError::Integrity(
                    "idempotent_session_request_conflict",
                ));
            }
            let mut targets = vec![projection_target("core_domain.sessions", session_id)];
            if next_state == SessionState::Ended {
                let active_scene_id = row
                    .get::<Option<String>, _>("active_scene_id")
                    .ok_or(CoreDomainRepositoryError::Integrity("active_scene_missing"))?;
                targets.push(projection_target("public.scenes", &active_scene_id));
            }
            return self
                .commit_event(
                    metadata,
                    campaign_id,
                    session_id,
                    ("session", session_state_action(next_state)),
                    &existing_event,
                    targets,
                )
                .await;
        }
        if metadata.expected_version != current_version {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "session_expected_version",
            ));
        }
        let mut session = Session::scheduled(
            session_id,
            campaign_id,
            row.get::<String, _>("room_id"),
            row.get::<String, _>("scenario_id"),
        )?;
        session.state = current_state;
        session.active_scene_id = row
            .get::<Option<String>, _>("active_scene_id")
            .map(trpg_domain_core::domain_entities_value_objects::SceneId::new)
            .transpose()?;
        session.version = u64::try_from(current_version)
            .map_err(|_| CoreDomainRepositoryError::Integrity("session_version"))?;
        session.transition(next_state)?;
        if next_state == SessionState::Ended {
            let replay_events = self.load_campaign_events(campaign_id).await?;
            if !canonical_session_gameplay_is_terminal(&replay_events, campaign_id, session_id)? {
                return Err(CoreDomainRepositoryError::InvalidInput(
                    "session_gameplay_not_terminal",
                ));
            }
        }
        let event = CoreDomainEvent::SessionStateChanged {
            schema_version: CORE_EVENT_SCHEMA_VERSION,
            session_id: session_id.to_owned(),
            from: current_state,
            to: next_state,
            changed_at_unix_ms,
        };
        let mut projection_targets = vec![projection_target("core_domain.sessions", session_id)];
        if next_state == SessionState::Ended {
            let active_scene_id = row
                .get::<Option<String>, _>("active_scene_id")
                .ok_or(CoreDomainRepositoryError::Integrity("active_scene_missing"))?;
            projection_targets.push(projection_target("public.scenes", &active_scene_id));
        }
        let persisted = self
            .commit_event(
                metadata,
                campaign_id,
                session_id,
                ("session", session_state_action(next_state)),
                &event,
                projection_targets,
            )
            .await?;
        if next_state == SessionState::Ended {
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
                 WHERE scene_id = $7
                   AND state = 'ACTIVE'
                "#,
            )
            .bind(&metadata.visibility_label)
            .bind(&metadata.visibility_subject)
            .bind(&metadata.provenance_kind)
            .bind(&metadata.provenance_reference)
            .bind(&metadata.provenance_recorded_by)
            .bind(persisted.last_event_sequence)
            .bind(row.get::<Option<String>, _>("active_scene_id"))
            .execute(&mut *transaction)
            .await
            .map_err(database_error("close_ending_scene"))?;
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
               AND version = $11
            "#,
        )
        .bind(next_state.as_str())
        .bind(changed_at)
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(persisted.last_event_sequence)
        .bind(session_id)
        .bind(current_state.as_str())
        .bind(current_version)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("project_session_transition"))?;
        if result.rows_affected() != 1 {
            return Err(CoreDomainRepositoryError::Integrity(
                "session_transition_projection_conflict",
            ));
        }
        transaction
            .commit()
            .await
            .map_err(database_error("commit_session_transition"))?;
        Ok(persisted)
    }
}
