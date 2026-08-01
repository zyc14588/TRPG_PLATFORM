impl CoreDomainRepository {
    pub async fn join_character_session(
        &self,
        metadata: &CoreCommandMetadata,
        request: &JoinCharacterSessionRequest,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        if metadata.expected_version != 0
            || metadata.requesting_actor_id != request.owner_user_id
            || request.joined_at_unix_ms == 0
        {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "character_session_join",
            ));
        }
        self.ensure_campaign_member(&request.campaign_id, &request.owner_user_id)
            .await?;
        let joined_at =
            timestamp_from_unix_ms(request.joined_at_unix_ms, "character_session.joined_at")?;
        let state = sqlx::query(
            r#"
            SELECT character.campaign_id AS character_campaign_id,
                   character.owner_user_id, character.state AS character_state,
                   session.campaign_id AS session_campaign_id,
                   session.state AS session_state
              FROM public.characters AS character
              JOIN core_domain.sessions AS session
                ON session.session_id = $2
             WHERE character.character_id = $1
            "#,
        )
        .bind(&request.character_id)
        .bind(&request.session_id)
        .fetch_optional(&self.primary)
        .await
        .map_err(database_error("load_character_session_join_state"))?
        .ok_or(CoreDomainRepositoryError::NotFound(
            "character_or_session",
        ))?;
        if state.get::<String, _>("character_campaign_id") != request.campaign_id
            || state.get::<String, _>("session_campaign_id") != request.campaign_id
            || state.get::<String, _>("owner_user_id") != request.owner_user_id
        {
            return Err(CoreDomainRepositoryError::Forbidden);
        }
        if state.get::<String, _>("character_state") != "APPROVED"
            || !matches!(
                state.get::<String, _>("session_state").as_str(),
                "ACTIVE" | "PAUSED"
            )
        {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "character_session_join_state",
            ));
        }

        let event = CoreDomainEvent::CharacterJoinedSession {
            schema_version: CORE_EVENT_SCHEMA_VERSION,
            join_id: request.join_id.clone(),
            campaign_id: request.campaign_id.clone(),
            session_id: request.session_id.clone(),
            character_id: request.character_id.clone(),
            owner_user_id: request.owner_user_id.clone(),
            joined_at_unix_ms: request.joined_at_unix_ms,
        };
        let existing = sqlx::query(
            r#"
            SELECT join_id, session_id, character_id, owner_user_id,
                   last_event_sequence
              FROM core_domain.session_characters
             WHERE join_id = $1
                OR (session_id = $2 AND character_id = $3)
                OR (session_id = $2 AND owner_user_id = $4)
            "#,
        )
        .bind(&request.join_id)
        .bind(&request.session_id)
        .bind(&request.character_id)
        .bind(&request.owner_user_id)
        .fetch_optional(&self.primary)
        .await
        .map_err(database_error("load_existing_character_session_join"))?;
        if let Some(existing) = existing {
            let sequence: i64 = existing.get("last_event_sequence");
            if existing.get::<String, _>("join_id") == request.join_id
                && existing.get::<String, _>("session_id") == request.session_id
                && existing.get::<String, _>("character_id") == request.character_id
                && existing.get::<String, _>("owner_user_id") == request.owner_user_id
                && self.projection_matches_command(sequence, metadata).await?
            {
                let existing_event = self
                    .load_idempotent_core_event(
                        &request.campaign_id,
                        &request.join_id,
                        metadata,
                        "CharacterJoinedSession",
                    )
                    .await?
                    .ok_or(CoreDomainRepositoryError::Integrity(
                        "idempotent_character_session_join_missing",
                    ))?;
                if existing_event != event {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "idempotent_character_session_join_conflict",
                    ));
                }
                return self
                    .commit_event(
                        metadata,
                        &request.campaign_id,
                        &request.join_id,
                        ("session_character", "session_character.join"),
                        &event,
                        vec![projection_target(
                            "core_domain.session_characters",
                            &request.join_id,
                        )],
                    )
                    .await;
            }
            return Err(CoreDomainRepositoryError::Integrity(
                "character_session_join_conflict",
            ));
        }

        let persisted = self
            .commit_event(
                metadata,
                &request.campaign_id,
                &request.join_id,
                ("session_character", "session_character.join"),
                &event,
                vec![projection_target(
                    "core_domain.session_characters",
                    &request.join_id,
                )],
            )
            .await?;
        let mut transaction = self
            .begin_projection_transaction(&metadata.commit_id, "begin_character_session_join")
            .await?;
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
            "#,
        )
        .bind(&request.join_id)
        .bind(&request.campaign_id)
        .bind(&request.session_id)
        .bind(&request.character_id)
        .bind(&request.owner_user_id)
        .bind(joined_at)
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(persisted.last_event_sequence)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("insert_character_session_join"))?;
        transaction
            .commit()
            .await
            .map_err(database_error("commit_character_session_join"))?;
        Ok(persisted)
    }

}
