impl CoreDomainRepository {
    pub async fn request_campaign_export(
        &self,
        metadata: &CoreCommandMetadata,
        request: &RequestCampaignExportRequest,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        if metadata.expected_version != 0
            || metadata.requesting_actor_id != request.requested_by
            || !matches!(
                request.audience.as_str(),
                "PLAYER" | "KEEPER_PRIVATE" | "AUDIT" | "CAMPAIGN_ARCHIVE"
            )
            || request.requested_at_unix_ms == 0
            || (request.audience == "PLAYER"
                && (metadata.visibility_label != "private_to_player"
                    || metadata.visibility_subject != request.requested_by))
            || (request.audience != "PLAYER" && metadata.visibility_label != "keeper_only")
        {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "campaign_export_request",
            ));
        }
        if request.audience == "PLAYER" {
            self.ensure_campaign_member(&request.campaign_id, &request.requested_by)
                .await?;
        } else {
            self.ensure_campaign_admin(&request.campaign_id, &request.requested_by)
                .await?;
        }
        let requested_at =
            timestamp_from_unix_ms(request.requested_at_unix_ms, "campaign_export.requested_at")?;
        let event = CoreDomainEvent::CampaignExportRequested {
            schema_version: CORE_EVENT_SCHEMA_VERSION,
            export_id: request.export_id.clone(),
            campaign_id: request.campaign_id.clone(),
            requested_by: request.requested_by.clone(),
            audience: request.audience.clone(),
            requested_at_unix_ms: request.requested_at_unix_ms,
        };
        if let Some(existing_sequence) = sqlx::query_scalar::<_, i64>(
            "SELECT last_event_sequence FROM public.campaign_exports WHERE export_id = $1",
        )
        .bind(&request.export_id)
        .fetch_optional(&self.primary)
        .await
        .map_err(database_error("load_existing_campaign_export"))?
        {
            if self
                .projection_matches_command(existing_sequence, metadata)
                .await?
            {
                let existing_event = self
                    .load_idempotent_core_event(
                        &request.campaign_id,
                        &request.export_id,
                        metadata,
                        "CampaignExportRequested",
                    )
                    .await?
                    .ok_or(CoreDomainRepositoryError::Integrity(
                        "idempotent_campaign_export_missing",
                    ))?;
                if existing_event != event {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "idempotent_campaign_export_conflict",
                    ));
                }
                return self
                    .commit_event(
                        metadata,
                        &request.campaign_id,
                        &request.export_id,
                        ("campaign_export", "campaign_export.request"),
                        &event,
                        vec![projection_target(
                            "public.campaign_exports",
                            &request.export_id,
                        )],
                    )
                    .await;
            }
            return Err(CoreDomainRepositoryError::Integrity(
                "campaign_export_identity_conflict",
            ));
        }

        let persisted = self
            .commit_event(
                metadata,
                &request.campaign_id,
                &request.export_id,
                ("campaign_export", "campaign_export.request"),
                &event,
                vec![projection_target(
                    "public.campaign_exports",
                    &request.export_id,
                )],
            )
            .await?;
        let mut transaction = self
            .begin_projection_transaction(&metadata.commit_id, "begin_campaign_export_request")
            .await?;
        sqlx::query(
            r#"
            INSERT INTO public.campaign_exports (
                export_id, campaign_id, requested_by, audience, state,
                requested_at, version,
                visibility_label, visibility_subject,
                provenance_kind, provenance_reference, provenance_recorded_by,
                last_event_sequence
            ) VALUES (
                $1, $2, $3, $4, 'REQUESTED', $5, 1,
                $6, $7, $8, $9, $10, $11
            )
            "#,
        )
        .bind(&request.export_id)
        .bind(&request.campaign_id)
        .bind(&request.requested_by)
        .bind(&request.audience)
        .bind(requested_at)
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(persisted.last_event_sequence)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("insert_campaign_export"))?;
        sqlx::query(
            r#"
            INSERT INTO public.campaign_export_jobs (
                export_id, campaign_id, state, requested_event_sequence
            ) VALUES ($1, $2, 'REQUESTED', $3)
            "#,
        )
        .bind(&request.export_id)
        .bind(&request.campaign_id)
        .bind(persisted.last_event_sequence)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("insert_campaign_export_job"))?;
        transaction
            .commit()
            .await
            .map_err(database_error("commit_campaign_export_request"))?;
        Ok(persisted)
    }

}
