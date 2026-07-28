
impl CoreDomainRepository {
    async fn campaign_invite_acceptance_projection_id(
        &self,
        projection: &serde_json::Value,
    ) -> Result<String, CoreDomainRepositoryError> {
        sqlx::query_scalar("SELECT core_domain.campaign_invite_acceptance_projection_id($1::JSONB)")
            .bind(sqlx::types::Json(projection.clone()))
            .fetch_one(&self.primary)
            .await
            .map_err(database_error(
                "derive_campaign_invite_acceptance_projection_id",
            ))
    }

    async fn player_action_projection_id(
        &self,
        projection: &serde_json::Value,
    ) -> Result<String, CoreDomainRepositoryError> {
        sqlx::query_scalar("SELECT core_domain.player_action_projection_id($1::JSONB)")
            .bind(sqlx::types::Json(projection.clone()))
            .fetch_one(&self.primary)
            .await
            .map_err(database_error("derive_player_action_projection_id"))
    }

    async fn gameplay_roll_reservation_projection_id(
        &self,
        projection: &serde_json::Value,
    ) -> Result<String, CoreDomainRepositoryError> {
        sqlx::query_scalar("SELECT core_domain.gameplay_roll_reservation_projection_id($1::JSONB)")
            .bind(sqlx::types::Json(projection.clone()))
            .fetch_one(&self.primary)
            .await
            .map_err(database_error(
                "derive_gameplay_roll_reservation_projection_id",
            ))
    }

    async fn session_ending_reservation_projection_id(
        &self,
        projection: &serde_json::Value,
    ) -> Result<String, CoreDomainRepositoryError> {
        sqlx::query_scalar("SELECT core_domain.session_ending_reservation_projection_id($1::JSONB)")
            .bind(sqlx::types::Json(projection.clone()))
            .fetch_one(&self.primary)
            .await
            .map_err(database_error(
                "derive_session_ending_reservation_projection_id",
            ))
    }

    async fn verify_player_action_commit(
        &self,
        metadata: &CoreCommandMetadata,
        action_id: &str,
        expected_state: &str,
        persisted: &PersistedCommit,
    ) -> Result<(), CoreDomainRepositoryError> {
        let valid: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1
                  FROM public.player_actions AS action
                  JOIN public.formal_commits AS formal
                    ON formal.commit_id = $1
                  JOIN public.event_store AS event
                    ON event.sequence BETWEEN
                       formal.first_event_sequence AND formal.last_event_sequence
                 WHERE action.action_id = $2
                   AND action.state = $3
                   AND formal.idempotency_key = $4
                   AND event.command_id = $5
                   AND formal.status = 'committed'
                   AND event.campaign_id = action.campaign_id
                   AND event.stream_id = action.action_id
                   AND event.integrity_status = 'verified_hmac'
                   AND event.event_integrity_version = 3
                   AND action.last_event_sequence BETWEEN
                       formal.first_event_sequence AND formal.last_event_sequence
            )
            "#,
        )
        .bind(&metadata.commit_id)
        .bind(action_id)
        .bind(expected_state)
        .bind(&metadata.idempotency_key)
        .bind(&metadata.command_id)
        .fetch_one(&self.primary)
        .await
        .map_err(database_error("verify_player_action_commit"))?;
        if !valid || persisted.commit_id != metadata.commit_id {
            return Err(CoreDomainRepositoryError::Integrity(
                "player_action_commit_projection_mismatch",
            ));
        }
        Ok(())
    }

    pub async fn submit_player_action(
        &self,
        metadata: &CoreCommandMetadata,
        request: &SubmitPlayerActionRequest,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        request.intent.validate()?;
        for value in [
            request.action_id.as_str(),
            request.campaign_id.as_str(),
            request.character_id.as_str(),
            request.scene_id.as_str(),
            request.submitted_by.as_str(),
        ] {
            EntityId::new(value)
                .map_err(|_| CoreDomainRepositoryError::InvalidInput("player_action_id"))?;
        }
        if metadata.expected_version != 0
            || metadata.requesting_actor_id != request.submitted_by
            || metadata.authority_mode != "human_kp"
            || request.submitted_at_unix_ms == 0
        {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "player_action_submission",
            ));
        }
        self.ensure_campaign_member(&request.campaign_id, &request.submitted_by)
            .await?;
        let authorized: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1
                  FROM public.characters AS character
                  JOIN public.scenes AS scene
                    ON scene.scene_id = $1
                   AND scene.campaign_id = character.campaign_id
                 WHERE character.character_id = $2
                   AND character.campaign_id = $3
                   AND character.owner_user_id = $4
                   AND character.state = 'APPROVED'
                   AND scene.state = 'ACTIVE'
            )
            "#,
        )
        .bind(&request.scene_id)
        .bind(&request.character_id)
        .bind(&request.campaign_id)
        .bind(&request.submitted_by)
        .fetch_one(&self.primary)
        .await
        .map_err(database_error("authorize_player_action_subjects"))?;
        if !authorized {
            return Err(CoreDomainRepositoryError::Forbidden);
        }

        let intent = serde_json::to_value(&request.intent)
            .map_err(|_| CoreDomainRepositoryError::Serialization)?;
        let projection = serde_json::json!({
            "kind": "SUBMIT",
            "action_id": request.action_id,
            "campaign_id": request.campaign_id,
            "character_id": request.character_id,
            "scene_id": request.scene_id,
            "submitted_by": request.submitted_by,
            "action_kind": request.intent.kind_name(),
            "intent": intent,
            "submitted_at_unix_ms": request.submitted_at_unix_ms,
            "visibility_label": metadata.visibility_label,
            "visibility_subject": metadata.visibility_subject,
            "provenance_kind": metadata.provenance_kind,
            "provenance_reference": metadata.provenance_reference,
            "provenance_recorded_by": metadata.provenance_recorded_by,
        });
        let projection_id = self.player_action_projection_id(&projection).await?;
        let event = player_action_event(
            "PlayerActionSubmitted",
            serde_json::json!({
                "schema_version": 1,
                "action_id": request.action_id,
                "campaign_id": request.campaign_id,
                "character_id": request.character_id,
                "scene_id": request.scene_id,
                "submitted_by": request.submitted_by,
                "intent": request.intent,
                "state": "AWAITING_HUMAN_CONFIRMATION",
            }),
            vec![
                projection_target("public.player_actions", &request.action_id),
                projection_target("core_domain.player_action_projection", &projection_id),
            ],
        )?;
        let draft = metadata.to_player_action_draft(
            &request.campaign_id,
            &request.action_id,
            vec![event],
        )?;
        let persisted = self
            .canonical
            .commit_player_action_projection(&draft, &projection)
            .await?;
        self.verify_player_action_commit(
            metadata,
            &request.action_id,
            "AWAITING_HUMAN_CONFIRMATION",
            &persisted,
        )
        .await?;
        Ok(persisted)
    }

    pub async fn load_pending_player_action(
        &self,
        campaign_id: &str,
        action_id: &str,
    ) -> Result<PendingPlayerActionRecord, CoreDomainRepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT action.action_id, action.campaign_id, action.character_id,
                   action.scene_id, action.submitted_by, action.intent_json,
                   sheet.sheet_json, character.current_sheet_version
              FROM public.player_actions AS action
              JOIN public.characters AS character
                ON character.character_id = action.character_id
               AND character.campaign_id = action.campaign_id
              JOIN public.character_sheet_versions AS sheet
                ON sheet.character_id = character.character_id
               AND sheet.version = character.current_sheet_version
             WHERE action.campaign_id = $1
               AND action.action_id = $2
               AND action.state = 'AWAITING_HUMAN_CONFIRMATION'
             FOR SHARE OF action, character, sheet
            "#,
        )
        .bind(campaign_id)
        .bind(action_id)
        .fetch_optional(&self.primary)
        .await
        .map_err(database_error("load_pending_player_action"))?
        .ok_or(CoreDomainRepositoryError::NotFound("pending_player_action"))?;
        let intent: sqlx::types::Json<PlayerActionIntentRecord> = row.get("intent_json");
        let sheet: sqlx::types::Json<serde_json::Value> = row.get("sheet_json");
        Ok(PendingPlayerActionRecord {
            action_id: row.get("action_id"),
            campaign_id: row.get("campaign_id"),
            character_id: row.get("character_id"),
            scene_id: row.get("scene_id"),
            submitted_by: row.get("submitted_by"),
            intent: intent.0,
            character_sheet_json: serde_json::to_string(&sheet.0)
                .map_err(|_| CoreDomainRepositoryError::Serialization)?,
            character_sheet_version: row.get("current_sheet_version"),
        })
    }

    pub async fn load_player_action_header(
        &self,
        campaign_id: &str,
        action_id: &str,
    ) -> Result<PlayerActionHeader, CoreDomainRepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT action_kind, submitted_by, state
              FROM public.player_actions
             WHERE campaign_id = $1 AND action_id = $2
            "#,
        )
        .bind(campaign_id)
        .bind(action_id)
        .fetch_optional(&self.primary)
        .await
        .map_err(database_error("load_player_action_header"))?
        .ok_or(CoreDomainRepositoryError::NotFound("player_action"))?;
        Ok(PlayerActionHeader {
            action_kind: row.get("action_kind"),
            submitted_by: row.get("submitted_by"),
            state: row.get("state"),
        })
    }

    pub async fn load_resolved_player_action_receipt(
        &self,
        metadata: &CoreCommandMetadata,
        campaign_id: &str,
        action_id: &str,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT formal.commit_id, formal.first_event_sequence,
                   formal.last_event_sequence, formal.first_stream_version,
                   formal.last_stream_version, formal.audit_sequence,
                   formal.witness_prepare_sequence,
                   formal.witness_prepare_hash
              FROM public.formal_commits AS formal
              JOIN public.player_actions AS action
                ON action.campaign_id = formal.campaign_id
               AND action.action_id = formal.stream_id
             WHERE formal.commit_id = $1
               AND formal.idempotency_key = $2
               AND formal.campaign_id = $3
               AND formal.stream_id = $4
               AND formal.expected_version = 1
               AND formal.status = 'committed'
               AND action.state = 'RESOLVED'
               AND action.last_event_sequence BETWEEN
                   formal.first_event_sequence AND formal.last_event_sequence
               AND EXISTS (
                   SELECT 1
                     FROM public.event_store AS event
                    WHERE event.sequence BETWEEN
                          formal.first_event_sequence AND formal.last_event_sequence
                      AND event.command_id = $5
                      AND event.event_type = 'DecisionCommitted'
                      AND event.integrity_status = 'verified_hmac'
               )
            "#,
        )
        .bind(&metadata.commit_id)
        .bind(&metadata.idempotency_key)
        .bind(campaign_id)
        .bind(action_id)
        .bind(&metadata.command_id)
        .fetch_optional(&self.primary)
        .await
        .map_err(database_error("load_resolved_player_action_receipt"))?
        .ok_or(CoreDomainRepositoryError::NotFound(
            "resolved_player_action_receipt",
        ))?;
        Ok(PersistedCommit {
            commit_id: row.get("commit_id"),
            first_event_sequence: row.get("first_event_sequence"),
            last_event_sequence: row.get("last_event_sequence"),
            first_stream_version: row.get("first_stream_version"),
            last_stream_version: row.get("last_stream_version"),
            audit_sequence: row.get("audit_sequence"),
            witness_prepare_sequence: row.get("witness_prepare_sequence"),
            witness_prepare_hash: row.get("witness_prepare_hash"),
        })
    }
}
