
impl CoreDomainRepository {

    pub async fn approve_character_initial_version(
        &self,
        metadata: &CoreCommandMetadata,
        campaign_id: &str,
        character_id: &str,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        self.ensure_campaign_admin(campaign_id, &metadata.requesting_actor_id)
            .await?;
        let row = sqlx::query(
            r#"
            SELECT campaign_id, owner_user_id, display_name, state,
                   current_sheet_version, initial_version_locked, version,
                   last_event_sequence,
                   (
                       SELECT sheet_version_id
                         FROM public.character_sheet_versions
                        WHERE character_id = $1 AND version = 1
                   ) AS initial_sheet_version_id
              FROM public.characters
             WHERE character_id = $1
            "#,
        )
        .bind(character_id)
        .fetch_optional(&self.primary)
        .await
        .map_err(database_error("load_character_for_review"))?
        .ok_or(CoreDomainRepositoryError::NotFound("character"))?;
        if row.get::<String, _>("campaign_id") != campaign_id {
            return Err(CoreDomainRepositoryError::Forbidden);
        }
        let current_version: i64 = row.get("version");
        let event = CoreDomainEvent::CharacterInitialVersionApproved {
            schema_version: CORE_EVENT_SCHEMA_VERSION,
            character_id: character_id.to_owned(),
            reviewed_by: metadata.requesting_actor_id.clone(),
        };
        let initial_sheet_version_id = row
            .get::<Option<String>, _>("initial_sheet_version_id")
            .ok_or(CoreDomainRepositoryError::Integrity(
                "initial_character_sheet_missing",
            ))?;
        let projection_targets = || {
            vec![
                projection_target("public.characters", character_id),
                projection_target("public.character_sheet_versions", &initial_sheet_version_id),
            ]
        };
        let current_state: String = row.get("state");
        let current_event_sequence: i64 = row.get("last_event_sequence");
        if current_state != "SUBMITTED" {
            if current_state == "APPROVED"
                && row.get::<bool, _>("initial_version_locked")
                && self
                    .projection_matches_command(current_event_sequence, metadata)
                    .await?
            {
                return self
                    .commit_event(
                        metadata,
                        campaign_id,
                        character_id,
                        ("character", "character.review_initial"),
                        &event,
                        projection_targets(),
                    )
                    .await;
            }
            return Err(CoreDomainRepositoryError::Domain(
                CoreEntityError::CharacterSheetNotSubmitted,
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
        character.state = CharacterState::Submitted;
        character.current_sheet_version = u64::try_from(row.get::<i64, _>("current_sheet_version"))
            .map_err(|_| CoreDomainRepositoryError::Integrity("character_version"))?;
        character.initial_version_locked = row.get("initial_version_locked");
        character.version = u64::try_from(current_version)
            .map_err(|_| CoreDomainRepositoryError::Integrity("character_version"))?;
        character.approve_initial_version()?;

        let persisted = self
            .commit_event(
                metadata,
                campaign_id,
                character_id,
                ("character", "character.review_initial"),
                &event,
                projection_targets(),
            )
            .await?;
        let mut transaction = self
            .begin_projection_transaction(&metadata.commit_id, "begin_character_review")
            .await?;
        let result = sqlx::query(
            r#"
            UPDATE public.characters
               SET state = 'APPROVED',
                   initial_version_locked = TRUE,
                   version = version + 1,
                   visibility_label = $1,
                   visibility_subject = $2,
                   provenance_kind = $3,
                   provenance_reference = $4,
                   provenance_recorded_by = $5,
                   last_event_sequence = $6
             WHERE character_id = $7
               AND state = 'SUBMITTED'
               AND initial_version_locked = FALSE
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
        .map_err(database_error("project_character_review"))?;
        if result.rows_affected() != 1 {
            return Err(CoreDomainRepositoryError::Integrity(
                "character_review_projection_conflict",
            ));
        }
        sqlx::query(
            r#"
            UPDATE public.character_sheet_versions
               SET locked = TRUE,
                   visibility_label = $1,
                   visibility_subject = $2,
                   provenance_kind = $3,
                   provenance_reference = $4,
                   provenance_recorded_by = $5,
                   last_event_sequence = $6
             WHERE character_id = $7
               AND version = 1
               AND locked = FALSE
            "#,
        )
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(persisted.last_event_sequence)
        .bind(character_id)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("lock_initial_character_sheet"))?;
        transaction
            .commit()
            .await
            .map_err(database_error("commit_character_review"))?;
        Ok(persisted)
    }

    pub async fn import_scenario(
        &self,
        metadata: &CoreCommandMetadata,
        request: &ImportScenarioRequest,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        if metadata.expected_version != 0
            || request.ruleset_id.trim().is_empty()
            || request.format_version.trim().is_empty()
            || !valid_sha256(&request.content_hash)
        {
            return Err(CoreDomainRepositoryError::InvalidInput("scenario"));
        }
        self.ensure_campaign_admin(&request.campaign_id, &metadata.requesting_actor_id)
            .await?;
        let document = validated_object(&request.document_json, "scenario.document_json")?;
        let canonical_document = serde_json::to_string(&document)
            .map_err(|_| CoreDomainRepositoryError::Serialization)?;
        let actual_hash = format!("sha256:{:x}", Sha256::digest(canonical_document.as_bytes()));
        if actual_hash != request.content_hash {
            return Err(CoreDomainRepositoryError::Integrity(
                "scenario_content_hash_mismatch",
            ));
        }
        let event = CoreDomainEvent::ScenarioImported {
            schema_version: CORE_EVENT_SCHEMA_VERSION,
            scenario_id: request.scenario_id.clone(),
            campaign_id: request.campaign_id.clone(),
            ruleset_id: request.ruleset_id.clone(),
            format_version: request.format_version.clone(),
            content_hash: request.content_hash.clone(),
            document_json: canonical_document,
        };
        let persisted = self
            .commit_event(
                metadata,
                &request.campaign_id,
                &request.scenario_id,
                ("scenario", "scenario.import"),
                &event,
                vec![projection_target("public.scenarios", &request.scenario_id)],
            )
            .await?;
        let mut transaction = self
            .begin_projection_transaction(&metadata.commit_id, "begin_scenario_import")
            .await?;
        let result = sqlx::query(
            r#"
            INSERT INTO public.scenarios (
                scenario_id, campaign_id, ruleset_id, format_version,
                content_hash, document_json, validated, version,
                visibility_label, visibility_subject,
                provenance_kind, provenance_reference, provenance_recorded_by,
                last_event_sequence
            ) VALUES (
                $1, $2, $3, $4, $5, $6, TRUE, 1,
                $7, $8, $9, $10, $11, $12
            )
            ON CONFLICT (scenario_id) DO NOTHING
            "#,
        )
        .bind(&request.scenario_id)
        .bind(&request.campaign_id)
        .bind(&request.ruleset_id)
        .bind(&request.format_version)
        .bind(&request.content_hash)
        .bind(sqlx::types::Json(document))
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(persisted.last_event_sequence)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("insert_scenario"))?;
        if result.rows_affected() == 0 {
            let existing_sequence: i64 = sqlx::query_scalar(
                "SELECT last_event_sequence FROM public.scenarios WHERE scenario_id = $1",
            )
            .bind(&request.scenario_id)
            .fetch_one(&mut *transaction)
            .await
            .map_err(database_error("load_existing_scenario"))?;
            if existing_sequence != persisted.last_event_sequence
                || !self
                    .projection_matches_command(existing_sequence, metadata)
                    .await?
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "scenario_identity_conflict",
                ));
            }
        }
        transaction
            .commit()
            .await
            .map_err(database_error("commit_scenario_import"))?;
        Ok(persisted)
    }
}
