
impl CoreDomainRepository {

    async fn record_chase_state_inner(
        &self,
        metadata: &CoreCommandMetadata,
        request: &RecordChaseStateRequest,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        let next_version = metadata
            .expected_version
            .checked_add(1)
            .filter(|version| *version > 0)
            .ok_or(CoreDomainRepositoryError::InvalidInput("chase_version"))?;
        if request.state_json.is_empty() || request.state_json.len() > 1_048_576 {
            return Err(CoreDomainRepositoryError::InvalidInput("chase_state"));
        }
        let inspected = inspect_chase_state(&request.state_json)
            .map_err(|_| CoreDomainRepositoryError::InvalidInput("chase_state"))?;
        validate_chase_server_roll_evidence(&request.state_json, &request.participant_rolls)
            .map_err(|_| CoreDomainRepositoryError::InvalidInput("chase_roll_evidence"))?;
        let roll_consumptions = chase_gameplay_roll_consumptions(&request.state_json)
            .map_err(|_| CoreDomainRepositoryError::InvalidInput("chase_roll_evidence"))?;
        let chase_id = inspected.chase_id().to_owned();
        let status = inspected.status();
        let range_band = i16::from(inspected.range());
        let segment = i64::from(inspected.segment());
        if i64::try_from(inspected.version()).ok() != Some(next_version) {
            return Err(CoreDomainRepositoryError::InvalidInput("chase_state"));
        }
        self.ensure_campaign_admin(&request.campaign_id, &metadata.requesting_actor_id)
            .await?;
        let mut transaction = self
            .begin_projection_transaction(&metadata.commit_id, "begin_chase_state")
            .await?;
        self.lock_p08_projection_rebuild_scope(&mut transaction, &request.campaign_id)
            .await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(format!("p08-chase:{}:{}", request.campaign_id, chase_id))
            .execute(&mut *transaction)
            .await
            .map_err(database_error("lock_chase_state"))?;
        let event = CoreDomainEvent::ChaseStateRecorded {
            schema_version: CORE_EVENT_SCHEMA_VERSION,
            chase_id: chase_id.clone(),
            campaign_id: request.campaign_id.clone(),
            session_id: request.session_id.clone(),
            status: status.to_owned(),
            range_band: u8::try_from(range_band)
                .map_err(|_| CoreDomainRepositoryError::InvalidInput("chase_range"))?,
            segment: u64::try_from(segment)
                .map_err(|_| CoreDomainRepositoryError::InvalidInput("chase_segment"))?,
            version: u64::try_from(next_version)
                .map_err(|_| CoreDomainRepositoryError::InvalidInput("chase_version"))?,
            state_json: request.state_json.clone(),
        };
        let canonical_retry = if let Some((sequence, existing_event)) = self
            .load_idempotent_core_event_record(
                &request.campaign_id,
                &chase_id,
                metadata,
                "ChaseStateRecorded",
            )
            .await?
        {
            if existing_event != event {
                return Err(CoreDomainRepositoryError::Integrity(
                    "idempotent_chase_request_conflict",
                ));
            }
            Some((
                existing_event,
                self.load_gameplay_retry_projection_targets(sequence)
                    .await?,
            ))
        } else {
            None
        };
        let existing = sqlx::query(
            r#"
            SELECT campaign_id, session_id, state_json, version,
                   last_event_sequence
              FROM public.chase_states
             WHERE chase_id = $1
            "#,
        )
        .bind(&chase_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(database_error("load_chase_state_transition"))?;
        let previous_state = if let Some(row) = existing {
            if row.get::<String, _>("campaign_id") != request.campaign_id
                || row.get::<String, _>("session_id") != request.session_id
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "chase_state_identity_conflict",
                ));
            }
            let event_sequence: i64 = row.get("last_event_sequence");
            if self
                .projection_matches_command(event_sequence, metadata)
                .await?
            {
                let (existing_event, projection_targets) =
                    canonical_retry
                        .as_ref()
                        .ok_or(CoreDomainRepositoryError::Integrity(
                            "idempotent_chase_event_missing",
                        ))?;
                return self
                    .commit_gameplay_event(
                        metadata,
                        &request.campaign_id,
                        &chase_id,
                        ("chase_state", "chase.state.record"),
                        existing_event,
                        projection_targets.clone(),
                        "CHASE",
                        &roll_consumptions,
                    )
                    .await;
            }
            if row.get::<i64, _>("version") != metadata.expected_version {
                return Err(CoreDomainRepositoryError::Integrity(
                    "chase_state_projection_conflict",
                ));
            }
            Some(row.get::<Value, _>("state_json"))
        } else {
            None
        };
        if canonical_retry.is_none() {
            self.lock_active_gameplay_session(
                &mut transaction,
                &request.campaign_id,
                &request.session_id,
            )
            .await?;
        }
        if metadata.expected_version == 0 {
            self.validate_initial_chase_participants(
                &mut transaction,
                &request.campaign_id,
                &request.session_id,
                &chase_id,
                &request.state_json,
            )
            .await?;
        }
        if (metadata.expected_version == 0) != previous_state.is_none() {
            return Err(CoreDomainRepositoryError::Integrity(
                "chase_state_projection_conflict",
            ));
        }
        let previous_state_json = previous_state
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|_| CoreDomainRepositoryError::Serialization)?;
        let validated =
            validate_chase_state_transition(previous_state_json.as_deref(), &request.state_json)
                .map_err(|_| CoreDomainRepositoryError::InvalidInput("chase_transition"))?;
        if validated != inspected {
            return Err(CoreDomainRepositoryError::Integrity(
                "chase_state_validation_mismatch",
            ));
        }
        let projection_targets = canonical_retry
            .map(|(_, projection_targets)| projection_targets)
            .unwrap_or_else(|| {
                gameplay_state_projection_targets(
                    "public.chase_states",
                    &chase_id,
                    !roll_consumptions.is_empty(),
                )
            });
        self.lock_unconsumed_gameplay_rolls(&mut transaction, &roll_consumptions, metadata)
            .await?;
        let persisted = self
            .commit_gameplay_event(
                metadata,
                &request.campaign_id,
                &chase_id,
                ("chase_state", "chase.state.record"),
                &event,
                projection_targets,
                "CHASE",
                &roll_consumptions,
            )
            .await?;
        project_gameplay_roll_consumptions(
            &mut transaction,
            &roll_consumptions,
            &request.campaign_id,
            "CHASE",
            &chase_id,
            &metadata.visibility_label,
            &metadata.visibility_subject,
            &metadata.provenance_kind,
            &metadata.provenance_reference,
            &metadata.provenance_recorded_by,
            persisted.last_event_sequence,
        )
        .await?;
        let result = sqlx::query(
            r#"
            INSERT INTO public.chase_states (
                chase_id, campaign_id, session_id, status, range_band,
                segment, state_json, version,
                visibility_label, visibility_subject,
                provenance_kind, provenance_reference, provenance_recorded_by,
                last_event_sequence
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7::JSONB, $8,
                $9, $10, $11, $12, $13, $14
            )
            ON CONFLICT (chase_id) DO UPDATE
               SET status = EXCLUDED.status,
                   range_band = EXCLUDED.range_band,
                   segment = EXCLUDED.segment,
                   state_json = EXCLUDED.state_json,
                   version = EXCLUDED.version,
                   visibility_label = EXCLUDED.visibility_label,
                   visibility_subject = EXCLUDED.visibility_subject,
                   provenance_kind = EXCLUDED.provenance_kind,
                   provenance_reference = EXCLUDED.provenance_reference,
                   provenance_recorded_by = EXCLUDED.provenance_recorded_by,
                   last_event_sequence = EXCLUDED.last_event_sequence
             WHERE chase_states.campaign_id = EXCLUDED.campaign_id
               AND chase_states.session_id = EXCLUDED.session_id
               AND chase_states.status = 'ONGOING'
               AND chase_states.version = $15
            "#,
        )
        .bind(&chase_id)
        .bind(&request.campaign_id)
        .bind(&request.session_id)
        .bind(status)
        .bind(range_band)
        .bind(segment)
        .bind(&request.state_json)
        .bind(next_version)
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(persisted.last_event_sequence)
        .bind(metadata.expected_version)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("project_chase_state"))?;
        if result.rows_affected() != 1 {
            return Err(CoreDomainRepositoryError::Integrity(
                "chase_state_projection_conflict",
            ));
        }
        transaction
            .commit()
            .await
            .map_err(database_error("commit_chase_state"))?;
        Ok(persisted)
    }
}
