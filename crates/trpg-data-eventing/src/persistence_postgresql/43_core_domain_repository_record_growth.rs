
impl CoreDomainRepository {

    pub async fn record_growth(
        &self,
        metadata: &CoreCommandMetadata,
        request: &RecordGrowthRequest,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        if metadata.expected_version != 0
            || request.skill_name.trim().is_empty()
            || request.skill_name.len() > 128
        {
            return Err(CoreDomainRepositoryError::InvalidInput("growth"));
        }
        self.ensure_campaign_admin(&request.campaign_id, &metadata.requesting_actor_id)
            .await?;
        let improvement_check = request.growth_rolls.improvement_check();
        let improvement_check_roll = improvement_check.value();
        let increase = request.growth_rolls.increase();
        let increase_roll = increase.map(|roll| roll.value());
        let server_roll_id = improvement_check.roll_id().to_owned();
        let increase_roll_id = increase.map(|roll| roll.roll_id().to_owned());
        if increase_roll_id.as_deref() == Some(server_roll_id.as_str()) {
            return Err(CoreDomainRepositoryError::InvalidInput("growth_rolls"));
        }
        let roll_consumptions =
            growth_gameplay_roll_consumptions(&server_roll_id, increase_roll_id.as_deref())
                .map_err(|_| CoreDomainRepositoryError::InvalidInput("growth_rolls"))?;
        let mut transaction = self
            .begin_projection_transaction(&metadata.commit_id, "begin_growth")
            .await?;
        self.lock_p08_projection_rebuild_scope(&mut transaction, &request.campaign_id)
            .await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(format!(
                "p08-growth:{}:{}",
                request.campaign_id, request.character_id
            ))
            .execute(&mut *transaction)
            .await
            .map_err(database_error("lock_growth_character"))?;
        // Serialize every Growth identity check with all character-sheet
        // inserts until this projection commits. Cross-campaign callers cannot
        // pass an absence check concurrently for the same global IDs.
        sqlx::query(
            "LOCK TABLE public.character_sheet_versions \
             IN SHARE ROW EXCLUSIVE MODE",
        )
        .execute(&mut *transaction)
        .await
        .map_err(database_error("lock_growth_sheet_identity"))?;
        if let Some(persisted) = self
            .resolve_existing_growth_commit(
                &mut transaction,
                metadata,
                request,
                improvement_check_roll,
                increase_roll,
                &server_roll_id,
                &increase_roll_id,
                &roll_consumptions,
            )
            .await?
        {
            return Ok(persisted);
        }
        if let Some(existing_growth_event_id) = sqlx::query_scalar::<_, String>(
            r#"
            SELECT growth_event_id
              FROM public.growth_events
             WHERE ending_event_id = $1
               AND character_id = $2
               AND skill_name = $3
            "#,
        )
        .bind(&request.ending_event_id)
        .bind(&request.character_id)
        .bind(request.skill_name.trim())
        .fetch_optional(&mut *transaction)
        .await
        .map_err(database_error("load_existing_semantic_growth"))?
        {
            if existing_growth_event_id != request.growth_event_id {
                return Err(CoreDomainRepositoryError::Integrity(
                    "growth_skill_already_recorded",
                ));
            }
        }
        // The global writer lock above turns this absence check into an atomic
        // reservation through canonical append and projection commit.
        let sheet_identity_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS( \
                 SELECT 1 FROM public.character_sheet_versions \
                  WHERE sheet_version_id = $1 \
             )",
        )
        .bind(&request.new_sheet_version_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(database_error("check_growth_sheet_identity"))?;
        if sheet_identity_exists {
            return Err(CoreDomainRepositoryError::Integrity(
                "growth_sheet_identity_conflict",
            ));
        }
        let row = sqlx::query(
            r#"
            SELECT character.current_sheet_version,
                   character.version AS character_version,
                   character.visibility_label::TEXT AS character_visibility_label,
                   character.visibility_subject AS character_visibility_subject,
                   sheet.version AS sheet_version,
                   sheet.sheet_json,
                   sheet.visibility_label::TEXT AS sheet_visibility_label,
                   sheet.visibility_subject AS sheet_visibility_subject,
                   ending.ending_id,
                   scenario.document_json
              FROM public.characters AS character
              JOIN public.character_sheet_versions AS sheet
                ON sheet.character_id = character.character_id
               AND sheet.version = character.current_sheet_version
              JOIN public.ending_events AS ending
                ON ending.ending_event_id = $1
               AND ending.campaign_id = character.campaign_id
               AND ending.session_id = $2
              JOIN core_domain.sessions AS session
                ON session.session_id = ending.session_id
               AND session.campaign_id = ending.campaign_id
              JOIN public.scenarios AS scenario
                ON scenario.scenario_id = session.scenario_id
               AND scenario.campaign_id = session.campaign_id
             WHERE character.character_id = $3
               AND character.campaign_id = $4
               AND sheet.sheet_version_id = $5
               AND sheet.locked
            "#,
        )
        .bind(&request.ending_event_id)
        .bind(&request.session_id)
        .bind(&request.character_id)
        .bind(&request.campaign_id)
        .bind(&request.source_sheet_version_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(database_error("load_growth_source"))?
        .ok_or(CoreDomainRepositoryError::NotFound("growth_source"))?;
        let source_version: i64 = row.get("sheet_version");
        let character_version: i64 = row.get("character_version");
        if row.get::<i64, _>("current_sheet_version") != source_version {
            return Err(CoreDomainRepositoryError::Integrity(
                "growth_source_not_current",
            ));
        }
        let character_visibility_label: String = row.get("character_visibility_label");
        let character_visibility_subject: String = row.get("character_visibility_subject");
        let sheet_visibility_label: String = row.get("sheet_visibility_label");
        let sheet_visibility_subject: String = row.get("sheet_visibility_subject");
        if character_visibility_label != sheet_visibility_label
            || character_visibility_subject != sheet_visibility_subject
        {
            return Err(CoreDomainRepositoryError::Integrity(
                "growth_source_visibility_mismatch",
            ));
        }
        if metadata.visibility_label != character_visibility_label
            || metadata.visibility_subject != character_visibility_subject
        {
            return Err(CoreDomainRepositoryError::PolicyEvidenceMismatch);
        }
        let ending_id: String = row.get("ending_id");
        let scenario_document: Value = row.get("document_json");
        let growth_award = scenario_document
            .get("endings")
            .and_then(Value::as_array)
            .and_then(|endings| {
                endings.iter().find(|ending| {
                    ending.get("id").and_then(Value::as_str) == Some(ending_id.trim())
                })
            })
            .and_then(|ending| ending.get("growth_awards"))
            .and_then(Value::as_array)
            .and_then(|awards| {
                awards.iter().find(|award| {
                    award.get("skill_name").and_then(Value::as_str)
                        == Some(request.skill_name.trim())
                })
            });
        let Some(growth_award) = growth_award else {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "growth_skill_not_awarded",
            ));
        };
        if let Some(consumed_by_character_ids) = growth_award.get("consumed_by_character_ids") {
            let consumed_by_character_ids = consumed_by_character_ids.as_array().ok_or(
                CoreDomainRepositoryError::Integrity("growth_award_consumption_shape"),
            )?;
            let mut consumed_character_ids = BTreeSet::new();
            for character_id in consumed_by_character_ids {
                let character_id = character_id.as_str().filter(|character_id| {
                    !character_id.trim().is_empty() && *character_id == character_id.trim()
                });
                let Some(character_id) = character_id else {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "growth_award_consumption_shape",
                    ));
                };
                if !consumed_character_ids.insert(character_id) {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "growth_award_consumption_shape",
                    ));
                }
            }
            if consumed_character_ids.contains(request.character_id.as_str()) {
                return Err(CoreDomainRepositoryError::Integrity(
                    "growth_skill_already_recorded",
                ));
            }
        }
        let mut sheet_json: Value = row.get("sheet_json");
        let persisted_skill = sheet_json
            .get("skills")
            .and_then(Value::as_object)
            .and_then(|skills| skills.get(request.skill_name.trim()))
            .and_then(Value::as_i64);
        let skill_before = persisted_skill
            .and_then(|value| u8::try_from(value).ok())
            .filter(|value| *value <= 99)
            .ok_or(CoreDomainRepositoryError::Integrity(
                "growth_skill_source_invalid",
            ))?;
        let qualifies = skill_before < 99
            && (improvement_check_roll > skill_before || improvement_check_roll >= 96);
        if qualifies != increase.is_some() {
            return Err(CoreDomainRepositoryError::InvalidInput("growth_rolls"));
        }
        let skill_after = increase_roll
            .map(|roll| skill_before.saturating_add(roll).min(99))
            .unwrap_or(skill_before);
        sheet_json
            .get_mut("skills")
            .and_then(Value::as_object_mut)
            .ok_or(CoreDomainRepositoryError::Integrity(
                "growth_sheet_skills_missing",
            ))?
            .insert(
                request.skill_name.trim().to_owned(),
                Value::from(skill_after),
            );
        sync_combat_skill_target(
            &mut sheet_json,
            request.skill_name.trim(),
            skill_before,
            skill_after,
        )?;
        let new_sheet_version =
            source_version
                .checked_add(1)
                .ok_or(CoreDomainRepositoryError::Integrity(
                    "growth_sheet_version_overflow",
                ))?;
        let event = CoreDomainEvent::CharacterGrowthApplied {
            schema_version: CORE_EVENT_SCHEMA_VERSION,
            growth_event_id: request.growth_event_id.clone(),
            campaign_id: request.campaign_id.clone(),
            session_id: request.session_id.clone(),
            ending_event_id: request.ending_event_id.clone(),
            character_id: request.character_id.clone(),
            source_sheet_version_id: request.source_sheet_version_id.clone(),
            new_sheet_version_id: request.new_sheet_version_id.clone(),
            skill_name: request.skill_name.trim().to_owned(),
            skill_before,
            improvement_check_roll,
            increase_roll,
            skill_after,
            server_roll_id: server_roll_id.clone(),
            increase_roll_id: increase_roll_id.clone(),
        };
        self.lock_unconsumed_gameplay_rolls(&mut transaction, &roll_consumptions, metadata)
            .await?;
        let mut projection_targets = vec![
            projection_target("public.growth_events", &request.growth_event_id),
            projection_target(
                "public.character_sheet_versions",
                &request.new_sheet_version_id,
            ),
            projection_target("public.characters", &request.character_id),
        ];
        projection_targets.push(projection_target(
            "public.gameplay_roll_consumptions",
            &request.growth_event_id,
        ));
        let persisted = self
            .commit_gameplay_event(
                metadata,
                &request.campaign_id,
                &request.growth_event_id,
                ("growth", "growth.record"),
                &event,
                projection_targets,
                "GROWTH",
                &roll_consumptions,
            )
            .await?;
        project_gameplay_roll_consumptions(
            &mut transaction,
            &roll_consumptions,
            &request.campaign_id,
            "GROWTH",
            &request.growth_event_id,
            &metadata.visibility_label,
            &metadata.visibility_subject,
            &metadata.provenance_kind,
            &metadata.provenance_reference,
            &metadata.provenance_recorded_by,
            persisted.last_event_sequence,
        )
        .await?;
        let projection = GrowthProjectionContext {
            metadata,
            request,
            new_sheet_version,
            sheet_json: &sheet_json,
            last_event_sequence: persisted.last_event_sequence,
            source_version,
            character_version,
            skill_before,
            improvement_check_roll,
            increase_roll,
            skill_after,
            server_roll_id: &server_roll_id,
            increase_roll_id: &increase_roll_id,
        };
        project_growth_rows(&mut transaction, &projection).await?;
        transaction
            .commit()
            .await
            .map_err(database_error("commit_growth"))?;
        Ok(persisted)
    }
}
