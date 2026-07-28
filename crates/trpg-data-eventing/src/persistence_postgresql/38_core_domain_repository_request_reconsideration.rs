
impl CoreDomainRepository {

    pub async fn request_reconsideration(
        &self,
        metadata: &CoreCommandMetadata,
        request: &RequestReconsiderationRequest,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        if metadata.expected_version != 0
            || metadata.requesting_actor_id != request.requested_by
            || request.original_event_sequence <= 0
            || request.reason.trim().is_empty()
            || request.reason.len() > 512
        {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "reconsideration_request",
            ));
        }
        self.ensure_campaign_member(&request.campaign_id, &request.requested_by)
            .await?;
        let original_is_visible_and_canonical: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1
                  FROM public.event_store AS source_event
                 WHERE source_event.sequence = $1
                   AND source_event.campaign_id = $2
                   AND source_event.integrity_status = 'verified_hmac'
                   AND source_event.request_hash_source = 'formal_commit'
                   AND source_event.visibility_label = $4
                   AND source_event.visibility_subject = $5
                   AND source_event.data_subject_id = $6
                   AND (
                        source_event.visibility_label IN (
                            'public', 'party_visible', 'spectator_visible'
                        )
                        OR source_event.visibility_label IN (
                            'private_to_player', 'investigator_private'
                        )
                        AND source_event.visibility_subject = $3
                        AND source_event.data_subject_id = $3
                        OR source_event.visibility_label = 'keeper_only'
                        AND EXISTS(
                            SELECT 1
                              FROM public.campaigns
                             WHERE campaign_id = $2
                               AND owner_user_id = $3
                            UNION ALL
                            SELECT 1
                              FROM public.campaign_memberships
                             WHERE campaign_id = $2
                               AND user_id = $3
                               AND role IN ('CAMPAIGN_OWNER', 'HUMAN_KEEPER')
                               AND revoked_at IS NULL
                        )
                   )
            )
            "#,
        )
        .bind(request.original_event_sequence)
        .bind(&request.campaign_id)
        .bind(&request.requested_by)
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.data_subject_id)
        .fetch_one(&self.primary)
        .await
        .map_err(database_error("load_reconsideration_source_event"))?;
        if !original_is_visible_and_canonical {
            return Err(CoreDomainRepositoryError::NotFound(
                "reconsideration_source_event",
            ));
        }
        let event = CoreDomainEvent::ReconsiderationRequested {
            schema_version: CORE_EVENT_SCHEMA_VERSION,
            reconsideration_id: request.reconsideration_id.clone(),
            campaign_id: request.campaign_id.clone(),
            original_event_sequence: u64::try_from(request.original_event_sequence)
                .map_err(|_| CoreDomainRepositoryError::InvalidInput("original_event_sequence"))?,
            requested_by: request.requested_by.clone(),
            reason: request.reason.clone(),
        };
        let persisted = self
            .commit_event(
                metadata,
                &request.campaign_id,
                &request.reconsideration_id,
                ("reconsideration", "reconsideration.request"),
                &event,
                vec![projection_target(
                    "public.reconsiderations",
                    &request.reconsideration_id,
                )],
            )
            .await?;
        let mut transaction = self
            .begin_projection_transaction(&metadata.commit_id, "begin_reconsideration_request")
            .await?;
        self.lock_p08_projection_rebuild_scope(&mut transaction, &request.campaign_id)
            .await?;
        let event_chain = Value::Array(vec![Value::String(metadata.command_id.clone())]);
        let result = sqlx::query(
            r#"
            INSERT INTO public.reconsiderations (
                reconsideration_id, campaign_id, original_event_sequence,
                requested_by, reason, state, resolution, event_chain, version,
                review_workflow_version,
                visibility_label, visibility_subject,
                provenance_kind, provenance_reference, provenance_recorded_by,
                last_event_sequence
            ) VALUES (
                $1, $2, $3, $4, $5, 'REQUESTED', NULL, $6, 1,
                2,
                $7, $8, $9, $10, $11, $12
            )
            ON CONFLICT (reconsideration_id) DO NOTHING
            "#,
        )
        .bind(&request.reconsideration_id)
        .bind(&request.campaign_id)
        .bind(request.original_event_sequence)
        .bind(&request.requested_by)
        .bind(&request.reason)
        .bind(sqlx::types::Json(event_chain))
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(persisted.last_event_sequence)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("insert_reconsideration"))?;
        if result.rows_affected() == 0 {
            let existing_sequence: i64 = sqlx::query_scalar(
                "SELECT last_event_sequence FROM public.reconsiderations \
                 WHERE reconsideration_id = $1",
            )
            .bind(&request.reconsideration_id)
            .fetch_one(&mut *transaction)
            .await
            .map_err(database_error("load_existing_reconsideration"))?;
            if existing_sequence != persisted.last_event_sequence
                || !self
                    .projection_matches_command(existing_sequence, metadata)
                    .await?
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "reconsideration_identity_conflict",
                ));
            }
        }
        transaction
            .commit()
            .await
            .map_err(database_error("commit_reconsideration_request"))?;
        Ok(persisted)
    }

    pub async fn review_reconsideration(
        &self,
        metadata: &CoreCommandMetadata,
        request: &ReviewReconsiderationRequest,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        let normalized_review_summary = request.review_summary.trim();
        if request.review_event_id.trim().is_empty()
            || normalized_review_summary.is_empty()
            || normalized_review_summary.len() > 512
        {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "reconsideration_review",
            ));
        }
        self.ensure_campaign_admin(&request.campaign_id, &metadata.requesting_actor_id)
            .await?;
        let row = sqlx::query(
            r#"
            SELECT reconsideration.campaign_id, reconsideration.state,
                   reconsideration.version, reconsideration.last_event_sequence,
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
        .map_err(database_error("load_reconsideration_for_review"))?
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
        if self
            .projection_matches_command(current_event_sequence, metadata)
            .await?
        {
            let existing_event = self
                .load_idempotent_core_event(
                    &request.campaign_id,
                    &request.reconsideration_id,
                    metadata,
                    "ReconsiderationReviewed",
                )
                .await?
                .ok_or(CoreDomainRepositoryError::Integrity(
                    "idempotent_reconsideration_event_missing",
                ))?;
            if !matches!(
                &existing_event,
                CoreDomainEvent::ReconsiderationReviewed {
                    reconsideration_id,
                    review_event_id,
                    review_summary: persisted_review_summary,
                    ..
                } if reconsideration_id == &request.reconsideration_id
                    && review_event_id == &request.review_event_id
                    && persisted_review_summary == normalized_review_summary
            ) {
                return Err(CoreDomainRepositoryError::Integrity(
                    "idempotent_reconsideration_request_conflict",
                ));
            }
            return self
                .commit_event(
                    metadata,
                    &request.campaign_id,
                    &request.reconsideration_id,
                    ("reconsideration", "reconsideration.review"),
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
        if row.get::<String, _>("state") != "REQUESTED" {
            return Err(CoreDomainRepositoryError::Domain(
                CoreEntityError::EventChainInvalid,
            ));
        }
        let event = CoreDomainEvent::ReconsiderationReviewed {
            schema_version: CORE_EVENT_SCHEMA_VERSION,
            reconsideration_id: request.reconsideration_id.clone(),
            review_event_id: request.review_event_id.clone(),
            review_summary: normalized_review_summary.to_owned(),
        };
        let persisted = self
            .commit_event(
                metadata,
                &request.campaign_id,
                &request.reconsideration_id,
                ("reconsideration", "reconsideration.review"),
                &event,
                vec![projection_target(
                    "public.reconsiderations",
                    &request.reconsideration_id,
                )],
            )
            .await?;
        let mut transaction = self
            .begin_projection_transaction(&metadata.commit_id, "begin_reconsideration_review")
            .await?;
        self.lock_p08_projection_rebuild_scope(&mut transaction, &request.campaign_id)
            .await?;
        let result = sqlx::query(
            r#"
            UPDATE public.reconsiderations
               SET state = 'REVIEWED',
                   review_summary = $1,
                   event_chain = event_chain || jsonb_build_array($2::TEXT),
                   version = version + 1,
                   visibility_label = $3,
                   visibility_subject = $4,
                   provenance_kind = $5,
                   provenance_reference = $6,
                   provenance_recorded_by = $7,
                   last_event_sequence = $8
             WHERE reconsideration_id = $9
               AND state = 'REQUESTED'
               AND version = $10
            "#,
        )
        .bind(normalized_review_summary)
        .bind(&request.review_event_id)
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
        .map_err(database_error("project_reconsideration_review"))?;
        if result.rows_affected() != 1 {
            return Err(CoreDomainRepositoryError::Integrity(
                "reconsideration_projection_conflict",
            ));
        }
        transaction
            .commit()
            .await
            .map_err(database_error("commit_reconsideration_review"))?;
        Ok(persisted)
    }
}
