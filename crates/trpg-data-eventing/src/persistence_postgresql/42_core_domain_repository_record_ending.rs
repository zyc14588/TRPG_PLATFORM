
impl CoreDomainRepository {

    pub async fn record_ending(
        &self,
        metadata: &CoreCommandMetadata,
        request: &RecordEndingRequest,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        let normalized_ending_id = request.ending_id.trim();
        let normalized_summary = request.summary.trim();
        if metadata.expected_version != 0
            || normalized_ending_id.is_empty()
            || normalized_summary.is_empty()
            || normalized_summary.len() > 1_024
            || request.ended_at_unix_ms == 0
        {
            return Err(CoreDomainRepositoryError::InvalidInput("ending"));
        }
        let ended_at = timestamp_from_unix_ms(request.ended_at_unix_ms, "ending_timestamp")?;
        self.ensure_campaign_admin(&request.campaign_id, &metadata.requesting_actor_id)
            .await?;
        let mut transaction = self
            .begin_projection_transaction(&metadata.commit_id, "begin_ending")
            .await?;
        self.lock_p08_projection_rebuild_scope(&mut transaction, &request.campaign_id)
            .await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(format!(
                "p08-ending:{}:{}",
                request.campaign_id, request.session_id
            ))
            .execute(&mut *transaction)
            .await
            .map_err(database_error("lock_ending_session"))?;
        let session = sqlx::query(
            r#"
            SELECT session.state, scenario.document_json
              FROM core_domain.sessions AS session
              JOIN public.scenarios AS scenario
                ON scenario.scenario_id = session.scenario_id
               AND scenario.campaign_id = session.campaign_id
             WHERE session.session_id = $1
               AND session.campaign_id = $2
            "#,
        )
        .bind(&request.session_id)
        .bind(&request.campaign_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(database_error("load_ending_session"))?
        .ok_or(CoreDomainRepositoryError::NotFound("ending_session"))?;
        if session.get::<String, _>("state") != "ENDED" {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "ending_session_state",
            ));
        }
        let scenario_document: Value = session.get("document_json");
        let ending_is_defined = scenario_document
            .get("endings")
            .and_then(Value::as_array)
            .is_some_and(|endings| {
                endings.iter().any(|ending| {
                    ending.get("id").and_then(Value::as_str) == Some(normalized_ending_id)
                })
            });
        if !ending_is_defined {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "ending_id_not_defined",
            ));
        }
        // Coordinate this global ID with ordinary writes and replay inserts.
        // The advisory lock is held across the independent canonical append so
        // a conflicting ID cannot appear between validation and projection.
        lock_ending_projection_identity(&mut transaction, &request.ending_event_id).await?;
        if let Some(existing) = sqlx::query(
            r#"
            SELECT campaign_id, ending_event_id
              FROM core_domain.session_ending_reservations
             WHERE session_id = $1
            "#,
        )
        .bind(&request.session_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(database_error("load_session_ending_reservation"))?
        {
            if existing.get::<String, _>("campaign_id") != request.campaign_id
                || existing.get::<String, _>("ending_event_id") != request.ending_event_id
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "ending_session_already_recorded",
                ));
            }
        }
        if let Some(existing_ending_event_id) = sqlx::query_scalar::<_, String>(
            "SELECT ending_event_id FROM public.ending_events WHERE session_id = $1",
        )
        .bind(&request.session_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(database_error("load_existing_session_ending"))?
        {
            if existing_ending_event_id != request.ending_event_id {
                return Err(CoreDomainRepositoryError::Integrity(
                    "ending_session_already_recorded",
                ));
            }
        }
        if let Some(existing) = sqlx::query(
            r#"
            SELECT campaign_id, session_id, last_event_sequence
              FROM public.ending_events
             WHERE ending_event_id = $1
            "#,
        )
        .bind(&request.ending_event_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(database_error("load_existing_ending_identity"))?
        {
            let existing_sequence: i64 = existing.get("last_event_sequence");
            if existing.get::<String, _>("campaign_id") != request.campaign_id
                || existing.get::<String, _>("session_id") != request.session_id
                || !self
                    .projection_matches_command(existing_sequence, metadata)
                    .await?
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "ending_identity_conflict",
                ));
            }
        }
        let event = CoreDomainEvent::EndingRecorded {
            schema_version: CORE_EVENT_SCHEMA_VERSION,
            ending_event_id: request.ending_event_id.clone(),
            campaign_id: request.campaign_id.clone(),
            session_id: request.session_id.clone(),
            ending_id: normalized_ending_id.to_owned(),
            summary: normalized_summary.to_owned(),
            ended_at_unix_ms: request.ended_at_unix_ms,
        };
        let reservation = serde_json::json!({
            "campaign_id": request.campaign_id,
            "session_id": request.session_id,
            "ending_event_id": request.ending_event_id,
            "ending_id": normalized_ending_id,
            "summary": normalized_summary,
            "ended_at_unix_ms": request.ended_at_unix_ms,
            "visibility_label": metadata.visibility_label,
            "visibility_subject": metadata.visibility_subject,
            "provenance_kind": metadata.provenance_kind,
            "provenance_reference": metadata.provenance_reference,
            "provenance_recorded_by": metadata.provenance_recorded_by,
        });
        let persisted = self
            .commit_ending_event(metadata, request, &event, &reservation)
            .await?;
        if let Some(existing_sequence) = sqlx::query_scalar::<_, i64>(
            "SELECT last_event_sequence FROM public.ending_events WHERE ending_event_id = $1",
        )
        .bind(&request.ending_event_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(database_error("load_existing_ending"))?
        {
            if existing_sequence == persisted.last_event_sequence
                && self
                    .projection_matches_command(existing_sequence, metadata)
                    .await?
            {
                return Ok(persisted);
            }
            return Err(CoreDomainRepositoryError::Integrity(
                "ending_identity_conflict",
            ));
        }
        let result = sqlx::query(
            r#"
            INSERT INTO public.ending_events (
                ending_event_id, campaign_id, session_id, ending_id, summary,
                ended_at, version, visibility_label, visibility_subject,
                provenance_kind, provenance_reference, provenance_recorded_by,
                last_event_sequence
            ) VALUES (
                $1, $2, $3, $4, $5, $6, 1, $7, $8, $9, $10, $11, $12
            )
            ON CONFLICT (ending_event_id) DO NOTHING
            "#,
        )
        .bind(&request.ending_event_id)
        .bind(&request.campaign_id)
        .bind(&request.session_id)
        .bind(normalized_ending_id)
        .bind(normalized_summary)
        .bind(ended_at)
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(persisted.last_event_sequence)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("project_ending"))?;
        if result.rows_affected() != 1 {
            return Err(CoreDomainRepositoryError::Integrity(
                "ending_identity_conflict",
            ));
        }
        transaction
            .commit()
            .await
            .map_err(database_error("commit_ending"))?;
        Ok(persisted)
    }
}
