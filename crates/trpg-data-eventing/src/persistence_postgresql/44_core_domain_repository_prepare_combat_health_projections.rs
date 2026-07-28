
impl CoreDomainRepository {

    async fn prepare_combat_health_projections(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        campaign_id: &str,
        combat_id: &str,
        combat_version: i64,
        changes: &[CombatHealthChange],
        expected_updates: Option<&[CharacterCombatHealthUpdate]>,
    ) -> Result<Vec<PreparedCombatHealthProjection>, CoreDomainRepositoryError> {
        if expected_updates.is_some_and(|updates| updates.is_empty()) {
            return Ok(Vec::new());
        }
        let mut prepared = Vec::new();
        for change in changes {
            let expected = expected_updates.and_then(|updates| {
                updates
                    .iter()
                    .find(|update| update.character_id == change.character_id)
            });
            let row = if let Some(expected) = expected {
                let source_sheet_version =
                    i64::try_from(expected.source_sheet_version).map_err(|_| {
                        CoreDomainRepositoryError::Integrity("combat_health_source_sheet_version")
                    })?;
                sqlx::query(
                    r#"
                    SELECT character.version AS character_version,
                           character.current_sheet_version,
                           character.visibility_label::TEXT
                               AS character_visibility_label,
                           character.visibility_subject
                               AS character_visibility_subject,
                           sheet.sheet_version_id,
                           sheet.version AS sheet_version,
                           sheet.sheet_json,
                           sheet.locked,
                           sheet.visibility_label::TEXT
                               AS sheet_visibility_label,
                           sheet.visibility_subject AS sheet_visibility_subject
                      FROM public.characters AS character
                      JOIN public.character_sheet_versions AS sheet
                        ON sheet.character_id = character.character_id
                       AND sheet.version = $1
                       AND sheet.campaign_id = character.campaign_id
                     WHERE character.character_id = $2
                       AND character.campaign_id = $3
                     FOR UPDATE OF character, sheet
                    "#,
                )
                .bind(source_sheet_version)
                .bind(&change.character_id)
                .bind(campaign_id)
                .fetch_optional(&mut **transaction)
                .await
                .map_err(database_error("load_combat_health_retry_source"))?
            } else {
                sqlx::query(
                    r#"
                    SELECT character.version AS character_version,
                           character.current_sheet_version,
                           character.visibility_label::TEXT
                               AS character_visibility_label,
                           character.visibility_subject
                               AS character_visibility_subject,
                           sheet.sheet_version_id,
                           sheet.version AS sheet_version,
                           sheet.sheet_json,
                           sheet.locked,
                           sheet.visibility_label::TEXT
                               AS sheet_visibility_label,
                           sheet.visibility_subject AS sheet_visibility_subject
                      FROM public.characters AS character
                      JOIN public.character_sheet_versions AS sheet
                        ON sheet.character_id = character.character_id
                       AND sheet.version = character.current_sheet_version
                       AND sheet.campaign_id = character.campaign_id
                     WHERE character.character_id = $1
                       AND character.campaign_id = $2
                     FOR UPDATE OF character, sheet
                    "#,
                )
                .bind(&change.character_id)
                .bind(campaign_id)
                .fetch_optional(&mut **transaction)
                .await
                .map_err(database_error("load_combat_health_source"))?
            };
            let Some(row) = row else {
                if expected.is_some() {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "combat_health_retry_source_missing",
                    ));
                }
                // Scenario NPC health remains in the Combat aggregate. Only
                // persisted Character participants own sheet projections.
                continue;
            };
            let source_sheet_version: i64 = row.get("sheet_version");
            let source_character_version: i64 = row.get("character_version");
            let current_sheet_version: i64 = row.get("current_sheet_version");
            let source_sheet_version_id: String = row.get("sheet_version_id");
            if !row.get::<bool, _>("locked")
                || row.get::<String, _>("character_visibility_label")
                    != row.get::<String, _>("sheet_visibility_label")
                || row.get::<String, _>("character_visibility_subject")
                    != row.get::<String, _>("sheet_visibility_subject")
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "combat_health_source_envelope",
                ));
            }
            let new_sheet_version =
                source_sheet_version
                    .checked_add(1)
                    .ok_or(CoreDomainRepositoryError::Integrity(
                        "combat_health_sheet_version",
                    ))?;
            let new_sheet_version_id =
                combat_health_sheet_version_id(combat_id, &change.character_id, combat_version)?;
            let update = CharacterCombatHealthUpdate {
                character_id: change.character_id.clone(),
                new_sheet_version_id,
                source_sheet_version: u64::try_from(source_sheet_version).map_err(|_| {
                    CoreDomainRepositoryError::Integrity("combat_health_source_sheet_version")
                })?,
                source_character_version: u64::try_from(source_character_version).map_err(
                    |_| {
                        CoreDomainRepositoryError::Integrity(
                            "combat_health_source_character_version",
                        )
                    },
                )?,
                hp_before: change.hp_before,
                hp_after: change.hp_after,
                condition_before: change.condition_before.clone(),
                condition_after: change.condition_after.clone(),
            };
            if let Some(expected) = expected {
                if expected != &update
                    || current_sheet_version < source_sheet_version
                    || source_character_version
                        < i64::try_from(expected.source_character_version).map_err(|_| {
                            CoreDomainRepositoryError::Integrity(
                                "combat_health_source_character_version",
                            )
                        })?
                {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "combat_health_retry_update_mismatch",
                    ));
                }
            } else {
                if current_sheet_version != source_sheet_version {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "combat_health_source_not_current",
                    ));
                }
                let identity_conflict: bool = sqlx::query_scalar(
                    r#"
                    SELECT EXISTS(
                        SELECT 1
                          FROM public.character_sheet_versions
                         WHERE sheet_version_id = $1
                            OR character_id = $2 AND version = $3
                    )
                    "#,
                )
                .bind(&update.new_sheet_version_id)
                .bind(&change.character_id)
                .bind(new_sheet_version)
                .fetch_one(&mut **transaction)
                .await
                .map_err(database_error("check_combat_health_sheet_identity"))?;
                if identity_conflict {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "combat_health_sheet_identity_conflict",
                    ));
                }
            }
            let mut sheet_json: Value = row.get("sheet_json");
            let profile = sheet_json
                .get_mut("combat_profile")
                .and_then(Value::as_object_mut)
                .ok_or(CoreDomainRepositoryError::Integrity(
                    "combat_health_profile_missing",
                ))?;
            if profile
                .get("max_hp")
                .and_then(Value::as_u64)
                .is_none_or(|max_hp| u64::from(change.hp_after) > max_hp)
                || profile.get("current_hp").and_then(Value::as_u64).is_none()
                || profile.get("condition").and_then(Value::as_str).is_none()
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "combat_health_profile_shape",
                ));
            }
            profile.insert("current_hp".to_owned(), Value::from(change.hp_after));
            profile.insert(
                "condition".to_owned(),
                Value::String(change.condition_after.clone()),
            );
            prepared.push(PreparedCombatHealthProjection {
                update,
                source_sheet_version_id,
                sheet_json,
                visibility_label: row.get("sheet_visibility_label"),
                visibility_subject: row.get("sheet_visibility_subject"),
            });
        }
        if expected_updates.is_some_and(|updates| updates.len() != prepared.len()) {
            return Err(CoreDomainRepositoryError::Integrity(
                "combat_health_retry_update_mismatch",
            ));
        }
        Ok(prepared)
    }

    async fn project_combat_health_projections(
        transaction: &mut Transaction<'_, Postgres>,
        campaign_id: &str,
        metadata: &CoreCommandMetadata,
        event_sequence: i64,
        prepared: &[PreparedCombatHealthProjection],
    ) -> Result<(), CoreDomainRepositoryError> {
        for projection in prepared {
            let source_sheet_version = i64::try_from(projection.update.source_sheet_version)
                .map_err(|_| {
                    CoreDomainRepositoryError::Integrity("combat_health_source_sheet_version")
                })?;
            let source_character_version =
                i64::try_from(projection.update.source_character_version).map_err(|_| {
                    CoreDomainRepositoryError::Integrity("combat_health_source_character_version")
                })?;
            let new_sheet_version =
                source_sheet_version
                    .checked_add(1)
                    .ok_or(CoreDomainRepositoryError::Integrity(
                        "combat_health_sheet_version",
                    ))?;
            let inserted = sqlx::query(
                r#"
                INSERT INTO public.character_sheet_versions (
                    sheet_version_id, character_id, version, sheet_json, locked,
                    visibility_label, visibility_subject,
                    provenance_kind, provenance_reference,
                    provenance_recorded_by, campaign_id, last_event_sequence
                ) VALUES (
                    $1, $2, $3, $4, TRUE, $5, $6, $7, $8, $9, $10, $11
                )
                ON CONFLICT (sheet_version_id) DO NOTHING
                "#,
            )
            .bind(&projection.update.new_sheet_version_id)
            .bind(&projection.update.character_id)
            .bind(new_sheet_version)
            .bind(sqlx::types::Json(&projection.sheet_json))
            .bind(&projection.visibility_label)
            .bind(&projection.visibility_subject)
            .bind(&metadata.provenance_kind)
            .bind(&metadata.provenance_reference)
            .bind(&metadata.provenance_recorded_by)
            .bind(campaign_id)
            .bind(event_sequence)
            .execute(&mut **transaction)
            .await
            .map_err(database_error("project_combat_health_sheet"))?;
            if inserted.rows_affected() != 1 {
                return Err(CoreDomainRepositoryError::Integrity(
                    "combat_health_sheet_identity_conflict",
                ));
            }
            let advanced = sqlx::query(
                r#"
                UPDATE public.characters
                   SET current_sheet_version = $1,
                       version = $2,
                       provenance_kind = $3,
                       provenance_reference = $4,
                       provenance_recorded_by = $5,
                       last_event_sequence = $6
                 WHERE character_id = $7
                   AND campaign_id = $8
                   AND current_sheet_version = $9
                   AND version = $10
                   AND EXISTS (
                        SELECT 1
                          FROM public.character_sheet_versions AS source
                         WHERE source.sheet_version_id = $11
                           AND source.character_id = $7
                           AND source.version = $9
                   )
                "#,
            )
            .bind(new_sheet_version)
            .bind(source_character_version + 1)
            .bind(&metadata.provenance_kind)
            .bind(&metadata.provenance_reference)
            .bind(&metadata.provenance_recorded_by)
            .bind(event_sequence)
            .bind(&projection.update.character_id)
            .bind(campaign_id)
            .bind(source_sheet_version)
            .bind(source_character_version)
            .bind(&projection.source_sheet_version_id)
            .execute(&mut **transaction)
            .await
            .map_err(database_error("project_combat_health_character"))?;
            if advanced.rows_affected() != 1 {
                return Err(CoreDomainRepositoryError::Integrity(
                    "combat_health_character_projection_conflict",
                ));
            }
        }
        Ok(())
    }

    async fn lock_p08_projection_rebuild_scope(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        campaign_id: &str,
    ) -> Result<(), CoreDomainRepositoryError> {
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(format!("p08-projection-rebuild:{campaign_id}"))
            .execute(&mut **transaction)
            .await
            .map_err(database_error("lock_p08_projection_rebuild_scope"))?;
        Ok(())
    }
}
