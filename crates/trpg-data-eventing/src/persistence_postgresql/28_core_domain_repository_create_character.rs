
impl CoreDomainRepository {

    pub async fn create_character(
        &self,
        metadata: &CoreCommandMetadata,
        request: &CreateCharacterRequest,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        if metadata.expected_version != 0 || metadata.requesting_actor_id != request.owner_user_id {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "character_create_metadata",
            ));
        }
        self.ensure_campaign_member(&request.campaign_id, &request.owner_user_id)
            .await?;
        let character = Character::draft(
            &request.character_id,
            &request.campaign_id,
            &request.owner_user_id,
            &request.display_name,
        )?;
        let sheet_json = validated_object(&request.sheet_json, "character.sheet_json")?;
        let event = CoreDomainEvent::CharacterCreated {
            schema_version: CORE_EVENT_SCHEMA_VERSION,
            character_id: character.character_id.to_string(),
            campaign_id: character.campaign_id.to_string(),
            owner_user_id: character.owner_user_id.to_string(),
            display_name: character.display_name.clone(),
            sheet_version_id: request.sheet_version_id.clone(),
            sheet_json: serde_json::to_string(&sheet_json)
                .map_err(|_| CoreDomainRepositoryError::Serialization)?,
        };
        let persisted = self
            .commit_event(
                metadata,
                &request.campaign_id,
                &request.character_id,
                ("character", "character.create"),
                &event,
                vec![
                    projection_target("public.characters", &request.character_id),
                    projection_target("public.character_sheet_versions", &request.sheet_version_id),
                ],
            )
            .await?;

        if let Some(existing_sequence) = sqlx::query_scalar::<_, i64>(
            "SELECT last_event_sequence FROM public.characters WHERE character_id = $1",
        )
        .bind(&request.character_id)
        .fetch_optional(&self.primary)
        .await
        .map_err(database_error("load_existing_character"))?
        {
            if existing_sequence == persisted.last_event_sequence
                && self
                    .projection_matches_command(existing_sequence, metadata)
                    .await?
            {
                return Ok(persisted);
            }
            return Err(CoreDomainRepositoryError::Integrity(
                "character_identity_conflict",
            ));
        }

        let mut transaction = self
            .begin_projection_transaction(&metadata.commit_id, "begin_character_create")
            .await?;
        sqlx::query(
            r#"
            INSERT INTO public.characters (
                character_id, campaign_id, owner_user_id, display_name,
                state, current_sheet_version, initial_version_locked, version,
                visibility_label, visibility_subject,
                provenance_kind, provenance_reference, provenance_recorded_by,
                last_event_sequence
            ) VALUES (
                $1, $2, $3, $4, 'DRAFT', 1, FALSE, 1,
                $5, $6, $7, $8, $9, $10
            )
            "#,
        )
        .bind(&request.character_id)
        .bind(&request.campaign_id)
        .bind(&request.owner_user_id)
        .bind(&request.display_name)
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(persisted.last_event_sequence)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("insert_character"))?;
        sqlx::query(
            r#"
            INSERT INTO public.character_sheet_versions (
                sheet_version_id, character_id, version, sheet_json, locked,
                visibility_label, visibility_subject,
                provenance_kind, provenance_reference, provenance_recorded_by,
                campaign_id, last_event_sequence
            ) VALUES (
                $1, $2, 1, $3, FALSE, $4, $5, $6, $7, $8, $9, $10
            )
            "#,
        )
        .bind(&request.sheet_version_id)
        .bind(&request.character_id)
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
        .map_err(database_error("insert_character_sheet_version"))?;
        transaction
            .commit()
            .await
            .map_err(database_error("commit_character_create"))?;
        Ok(persisted)
    }

    pub async fn submit_character(
        &self,
        metadata: &CoreCommandMetadata,
        campaign_id: &str,
        character_id: &str,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT campaign_id, owner_user_id, display_name, state,
                   current_sheet_version, initial_version_locked, version,
                   last_event_sequence
              FROM public.characters
             WHERE character_id = $1
            "#,
        )
        .bind(character_id)
        .fetch_optional(&self.primary)
        .await
        .map_err(database_error("load_character_for_submit"))?
        .ok_or(CoreDomainRepositoryError::NotFound("character"))?;
        if row.get::<String, _>("campaign_id") != campaign_id
            || row.get::<String, _>("owner_user_id") != metadata.requesting_actor_id
        {
            return Err(CoreDomainRepositoryError::Forbidden);
        }
        let current_version: i64 = row.get("version");
        let event = CoreDomainEvent::CharacterSubmitted {
            schema_version: CORE_EVENT_SCHEMA_VERSION,
            character_id: character_id.to_owned(),
        };
        let current_state: String = row.get("state");
        let current_event_sequence: i64 = row.get("last_event_sequence");
        if current_state != "DRAFT" {
            if current_state == "SUBMITTED"
                && self
                    .projection_matches_command(current_event_sequence, metadata)
                    .await?
            {
                return self
                    .commit_event(
                        metadata,
                        campaign_id,
                        character_id,
                        ("character", "character.submit"),
                        &event,
                        vec![projection_target("public.characters", character_id)],
                    )
                    .await;
            }
            return Err(CoreDomainRepositoryError::Domain(
                CoreEntityError::CharacterSheetAlreadyLocked,
            ));
        }
        if metadata.expected_version != current_version {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "character_expected_version",
            ));
        }
        let mut character = Character::draft(
            character_id,
            campaign_id,
            row.get::<String, _>("owner_user_id"),
            row.get::<String, _>("display_name"),
        )?;
        character.current_sheet_version = u64::try_from(row.get::<i64, _>("current_sheet_version"))
            .map_err(|_| CoreDomainRepositoryError::Integrity("character_version"))?;
        character.initial_version_locked = row.get("initial_version_locked");
        character.version = u64::try_from(current_version)
            .map_err(|_| CoreDomainRepositoryError::Integrity("character_version"))?;
        character.submit()?;

        let persisted = self
            .commit_event(
                metadata,
                campaign_id,
                character_id,
                ("character", "character.submit"),
                &event,
                vec![projection_target("public.characters", character_id)],
            )
            .await?;
        let mut transaction = self
            .begin_projection_transaction(&metadata.commit_id, "begin_character_submit")
            .await?;
        let result = sqlx::query(
            r#"
            UPDATE public.characters
               SET state = 'SUBMITTED',
                   version = version + 1,
                   visibility_label = $1,
                   visibility_subject = $2,
                   provenance_kind = $3,
                   provenance_reference = $4,
                   provenance_recorded_by = $5,
                   last_event_sequence = $6
             WHERE character_id = $7
               AND state = 'DRAFT'
               AND version = $8
            "#,
        )
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(persisted.last_event_sequence)
        .bind(character_id)
        .bind(current_version)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("project_character_submit"))?;
        if result.rows_affected() != 1 {
            return Err(CoreDomainRepositoryError::Integrity(
                "character_submit_projection_conflict",
            ));
        }
        transaction
            .commit()
            .await
            .map_err(database_error("commit_character_submit"))?;
        Ok(persisted)
    }
}
