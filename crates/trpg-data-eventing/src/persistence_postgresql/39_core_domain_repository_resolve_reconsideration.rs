
impl CoreDomainRepository {

    pub async fn resolve_reconsideration(
        &self,
        metadata: &CoreCommandMetadata,
        request: &ResolveReconsiderationRequest,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        let normalized_resolution = request.resolution.trim();
        if request.resolution_event_id.trim().is_empty()
            || normalized_resolution.is_empty()
            || normalized_resolution.len() > 512
        {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "reconsideration_resolution",
            ));
        }
        let corrected_payload = match request.outcome {
            ReconsiderationOutcome::Upheld => {
                if request.corrected_event_type.is_some()
                    || request.corrected_payload_json.is_some()
                {
                    return Err(CoreDomainRepositoryError::InvalidInput(
                        "upheld_reconsideration_correction",
                    ));
                }
                None
            }
            ReconsiderationOutcome::Corrected => {
                let event_type = request
                    .corrected_event_type
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty() && value.len() <= 128)
                    .ok_or(CoreDomainRepositoryError::InvalidInput(
                        "corrected_event_type",
                    ))?;
                let payload_json = request.corrected_payload_json.as_deref().ok_or(
                    CoreDomainRepositoryError::InvalidInput("corrected_payload_json"),
                )?;
                let payload: Value = serde_json::from_str(payload_json).map_err(|_| {
                    CoreDomainRepositoryError::InvalidInput("corrected_payload_json")
                })?;
                if !payload.is_object() {
                    return Err(CoreDomainRepositoryError::InvalidInput(
                        "corrected_payload_json",
                    ));
                }
                Some((event_type.to_owned(), payload_json.to_owned()))
            }
        };
        self.ensure_campaign_admin(&request.campaign_id, &metadata.requesting_actor_id)
            .await?;
        let row = sqlx::query(
            r#"
            SELECT reconsideration.campaign_id,
                   reconsideration.original_event_sequence,
                   reconsideration.state, reconsideration.version,
                   reconsideration.last_event_sequence,
                   reconsideration.visibility_label::TEXT AS visibility_label,
                   reconsideration.visibility_subject,
                   chain_event.visibility_label AS event_visibility_label,
                   chain_event.visibility_subject AS event_visibility_subject,
                   chain_event.data_subject_id
              FROM public.reconsiderations AS reconsideration
              JOIN public.event_store AS chain_event
                ON chain_event.sequence = reconsideration.last_event_sequence
             WHERE reconsideration.reconsideration_id = $1
            "#,
        )
        .bind(&request.reconsideration_id)
        .fetch_optional(&self.primary)
        .await
        .map_err(database_error("load_reconsideration_for_resolution"))?
        .ok_or(CoreDomainRepositoryError::NotFound("reconsideration"))?;
        if row.get::<String, _>("campaign_id") != request.campaign_id {
            return Err(CoreDomainRepositoryError::Forbidden);
        }
        if row.get::<String, _>("visibility_label")
            != row.get::<String, _>("event_visibility_label")
            || row.get::<String, _>("visibility_subject")
                != row.get::<String, _>("event_visibility_subject")
        {
            return Err(CoreDomainRepositoryError::Integrity(
                "reconsideration_visibility_projection_mismatch",
            ));
        }
        if metadata.visibility_label != row.get::<String, _>("event_visibility_label")
            || metadata.visibility_subject != row.get::<String, _>("event_visibility_subject")
            || metadata.data_subject_id != row.get::<String, _>("data_subject_id")
        {
            return Err(CoreDomainRepositoryError::Forbidden);
        }
        let current_version: i64 = row.get("version");
        let current_event_sequence: i64 = row.get("last_event_sequence");
        let original_event_sequence: i64 = row.get("original_event_sequence");
        let event_type = match request.outcome {
            ReconsiderationOutcome::Upheld => "ReconsiderationUpheld",
            ReconsiderationOutcome::Corrected => "ReconsiderationCorrected",
        };
        if self
            .projection_matches_command(current_event_sequence, metadata)
            .await?
        {
            let existing_event = self
                .load_idempotent_core_event(
                    &request.campaign_id,
                    &request.reconsideration_id,
                    metadata,
                    event_type,
                )
                .await?
                .ok_or(CoreDomainRepositoryError::Integrity(
                    "idempotent_reconsideration_resolution_event_missing",
                ))?;
            let matches_request = match (&existing_event, request.outcome, &corrected_payload) {
                (
                    CoreDomainEvent::ReconsiderationUpheld {
                        reconsideration_id,
                        resolution_event_id,
                        original_event_sequence: persisted_original,
                        resolution: persisted_resolution,
                        ..
                    },
                    ReconsiderationOutcome::Upheld,
                    None,
                ) => {
                    reconsideration_id == &request.reconsideration_id
                        && resolution_event_id == &request.resolution_event_id
                        && *persisted_original
                            == u64::try_from(original_event_sequence).unwrap_or_default()
                        && persisted_resolution == normalized_resolution
                }
                (
                    CoreDomainEvent::ReconsiderationCorrected {
                        reconsideration_id,
                        resolution_event_id,
                        original_event_sequence: persisted_original,
                        resolution: persisted_resolution,
                        corrected_event_type,
                        corrected_payload_json,
                        ..
                    },
                    ReconsiderationOutcome::Corrected,
                    Some((requested_event_type, requested_payload)),
                ) => {
                    reconsideration_id == &request.reconsideration_id
                        && resolution_event_id == &request.resolution_event_id
                        && *persisted_original
                            == u64::try_from(original_event_sequence).unwrap_or_default()
                        && persisted_resolution == normalized_resolution
                        && corrected_event_type == requested_event_type
                        && corrected_payload_json == requested_payload
                }
                _ => false,
            };
            if !matches_request {
                return Err(CoreDomainRepositoryError::Integrity(
                    "idempotent_reconsideration_resolution_conflict",
                ));
            }
            return self
                .commit_event(
                    metadata,
                    &request.campaign_id,
                    &request.reconsideration_id,
                    ("reconsideration", "reconsideration.resolve"),
                    &existing_event,
                    vec![projection_target(
                        "public.reconsiderations",
                        &request.reconsideration_id,
                    )],
                )
                .await;
        }
        if metadata.expected_version != current_version {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "reconsideration_expected_version",
            ));
        }
        if row.get::<String, _>("state") != "REVIEWED" {
            return Err(CoreDomainRepositoryError::Domain(
                CoreEntityError::EventChainInvalid,
            ));
        }
        let original_event_sequence = u64::try_from(original_event_sequence)
            .map_err(|_| CoreDomainRepositoryError::Integrity("original_event_sequence"))?;
        let event = match &corrected_payload {
            None => CoreDomainEvent::ReconsiderationUpheld {
                schema_version: CORE_EVENT_SCHEMA_VERSION,
                reconsideration_id: request.reconsideration_id.clone(),
                resolution_event_id: request.resolution_event_id.clone(),
                original_event_sequence,
                resolution: normalized_resolution.to_owned(),
            },
            Some((corrected_event_type, corrected_payload_json)) => {
                CoreDomainEvent::ReconsiderationCorrected {
                    schema_version: CORE_EVENT_SCHEMA_VERSION,
                    reconsideration_id: request.reconsideration_id.clone(),
                    resolution_event_id: request.resolution_event_id.clone(),
                    original_event_sequence,
                    resolution: normalized_resolution.to_owned(),
                    corrected_event_type: corrected_event_type.clone(),
                    corrected_payload_json: corrected_payload_json.clone(),
                }
            }
        };
        let persisted = self
            .commit_event(
                metadata,
                &request.campaign_id,
                &request.reconsideration_id,
                ("reconsideration", "reconsideration.resolve"),
                &event,
                vec![projection_target(
                    "public.reconsiderations",
                    &request.reconsideration_id,
                )],
            )
            .await?;
        let (outcome, corrected_event_type, corrected_payload_json) = match &corrected_payload {
            None => ("UPHELD", None, None),
            Some((event_type, payload_json)) => (
                "CORRECTED",
                Some(event_type.as_str()),
                Some(payload_json.as_str()),
            ),
        };
        let mut transaction = self
            .begin_projection_transaction(&metadata.commit_id, "begin_reconsideration_resolution")
            .await?;
        self.lock_p08_projection_rebuild_scope(&mut transaction, &request.campaign_id)
            .await?;
        let result = sqlx::query(
            r#"
            UPDATE public.reconsiderations
               SET state = 'RESOLVED',
                   outcome = $1,
                   resolution = $2,
                   corrected_event_type = $3,
                   corrected_payload = $4::JSONB,
                   event_chain = event_chain || jsonb_build_array($5::TEXT),
                   version = version + 1,
                   visibility_label = $6,
                   visibility_subject = $7,
                   provenance_kind = $8,
                   provenance_reference = $9,
                   provenance_recorded_by = $10,
                   last_event_sequence = $11
             WHERE reconsideration_id = $12
               AND state = 'REVIEWED'
               AND version = $13
            "#,
        )
        .bind(outcome)
        .bind(normalized_resolution)
        .bind(corrected_event_type)
        .bind(corrected_payload_json)
        .bind(&request.resolution_event_id)
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(persisted.last_event_sequence)
        .bind(&request.reconsideration_id)
        .bind(current_version)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("project_reconsideration_resolution"))?;
        if result.rows_affected() != 1 {
            return Err(CoreDomainRepositoryError::Integrity(
                "reconsideration_resolution_projection_conflict",
            ));
        }
        transaction
            .commit()
            .await
            .map_err(database_error("commit_reconsideration_resolution"))?;
        Ok(persisted)
    }

    pub async fn record_combat_state(
        &self,
        metadata: &CoreCommandMetadata,
        request: &RecordCombatStateRequest,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        Box::pin(self.record_combat_state_inner(metadata, request)).await
    }
}
