impl CoreDomainRepository {
    pub async fn update_character(
        &self,
        metadata: &CoreCommandMetadata,
        request: &UpdateCharacterRequest,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        if metadata.requesting_actor_id != request.owner_user_id
            || request.display_name.trim().is_empty()
            || request.display_name.len() > 512
        {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "character_update",
            ));
        }
        let sheet_json = validated_object(&request.sheet_json, "character.sheet_json")?;
        self.ensure_campaign_member(&request.campaign_id, &request.owner_user_id)
            .await?;
        let row = sqlx::query(
            r#"
            SELECT campaign_id, owner_user_id, state, current_sheet_version,
                   initial_version_locked, version, last_event_sequence
              FROM public.characters
             WHERE character_id = $1
            "#,
        )
        .bind(&request.character_id)
        .fetch_optional(&self.primary)
        .await
        .map_err(database_error("load_character_for_update"))?
        .ok_or(CoreDomainRepositoryError::NotFound("character"))?;
        if row.get::<String, _>("campaign_id") != request.campaign_id
            || row.get::<String, _>("owner_user_id") != request.owner_user_id
        {
            return Err(CoreDomainRepositoryError::Forbidden);
        }

        let event = CoreDomainEvent::CharacterUpdated {
            schema_version: CORE_EVENT_SCHEMA_VERSION,
            character_id: request.character_id.clone(),
            campaign_id: request.campaign_id.clone(),
            display_name: request.display_name.trim().to_owned(),
            sheet_version_id: request.sheet_version_id.clone(),
            sheet_json: serde_json::to_string(&sheet_json)
                .map_err(|_| CoreDomainRepositoryError::Serialization)?,
        };
        let current_event_sequence: i64 = row.get("last_event_sequence");
        if self
            .projection_matches_command(current_event_sequence, metadata)
            .await?
        {
            let existing_event = self
                .load_idempotent_core_event(
                    &request.campaign_id,
                    &request.character_id,
                    metadata,
                    "CharacterUpdated",
                )
                .await?
                .ok_or(CoreDomainRepositoryError::Integrity(
                    "idempotent_character_update_missing",
                ))?;
            if existing_event != event {
                return Err(CoreDomainRepositoryError::Integrity(
                    "idempotent_character_update_conflict",
                ));
            }
            return self
                .commit_event(
                    metadata,
                    &request.campaign_id,
                    &request.character_id,
                    ("character", "character.update"),
                    &event,
                    vec![
                        projection_target("public.characters", &request.character_id),
                        projection_target(
                            "public.character_sheet_versions",
                            &request.sheet_version_id,
                        ),
                    ],
                )
                .await;
        }

        let current_version: i64 = row.get("version");
        if metadata.expected_version != current_version {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "character_expected_version",
            ));
        }
        if row.get::<String, _>("state") != "DRAFT"
            || row.get::<bool, _>("initial_version_locked")
        {
            return Err(CoreDomainRepositoryError::Domain(
                CoreEntityError::CharacterSheetAlreadyLocked,
            ));
        }
        let next_sheet_version = row
            .get::<i64, _>("current_sheet_version")
            .checked_add(1)
            .ok_or(CoreDomainRepositoryError::Integrity(
                "character_sheet_version_overflow",
            ))?;
        let sheet_identity_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM public.character_sheet_versions \
             WHERE sheet_version_id = $1)",
        )
        .bind(&request.sheet_version_id)
        .fetch_one(&self.primary)
        .await
        .map_err(database_error("check_character_sheet_identity"))?;
        if sheet_identity_exists {
            return Err(CoreDomainRepositoryError::Integrity(
                "character_sheet_identity_conflict",
            ));
        }

        let persisted = self
            .commit_event(
                metadata,
                &request.campaign_id,
                &request.character_id,
                ("character", "character.update"),
                &event,
                vec![
                    projection_target("public.characters", &request.character_id),
                    projection_target(
                        "public.character_sheet_versions",
                        &request.sheet_version_id,
                    ),
                ],
            )
            .await?;
        let mut transaction = self
            .begin_projection_transaction(&metadata.commit_id, "begin_character_update")
            .await?;
        sqlx::query(
            r#"
            INSERT INTO public.character_sheet_versions (
                sheet_version_id, character_id, version, sheet_json, locked,
                visibility_label, visibility_subject,
                provenance_kind, provenance_reference, provenance_recorded_by,
                campaign_id, last_event_sequence
            ) VALUES (
                $1, $2, $3, $4, FALSE, $5, $6, $7, $8, $9, $10, $11
            )
            "#,
        )
        .bind(&request.sheet_version_id)
        .bind(&request.character_id)
        .bind(next_sheet_version)
        .bind(sqlx::types::Json(sheet_json))
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(&request.campaign_id)
        .bind(persisted.last_event_sequence)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("insert_updated_character_sheet"))?;
        let result = sqlx::query(
            r#"
            UPDATE public.characters
               SET display_name = $1,
                   current_sheet_version = $2,
                   version = version + 1,
                   visibility_label = $3,
                   visibility_subject = $4,
                   provenance_kind = $5,
                   provenance_reference = $6,
                   provenance_recorded_by = $7,
                   last_event_sequence = $8
             WHERE character_id = $9
               AND campaign_id = $10
               AND owner_user_id = $11
               AND state = 'DRAFT'
               AND initial_version_locked = FALSE
               AND version = $12
            "#,
        )
        .bind(request.display_name.trim())
        .bind(next_sheet_version)
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(persisted.last_event_sequence)
        .bind(&request.character_id)
        .bind(&request.campaign_id)
        .bind(&request.owner_user_id)
        .bind(current_version)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("project_character_update"))?;
        if result.rows_affected() != 1 {
            return Err(CoreDomainRepositoryError::Integrity(
                "character_update_projection_conflict",
            ));
        }
        transaction
            .commit()
            .await
            .map_err(database_error("commit_character_update"))?;
        Ok(persisted)
    }

}
