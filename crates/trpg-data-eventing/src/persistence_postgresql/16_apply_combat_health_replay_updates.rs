
async fn apply_combat_health_replay_updates(
    transaction: &mut Transaction<'_, Postgres>,
    replay: &CanonicalReplayEvent,
    combat_id: &str,
    combat_version: i64,
    next_state: &Value,
    changes: Option<&[CombatHealthChange]>,
    updates: &[CharacterCombatHealthUpdate],
) -> Result<(), CoreDomainRepositoryError> {
    let participants = combat_participant_values(
        &serde_json::to_string(next_state).map_err(|_| CoreDomainRepositoryError::Serialization)?,
    )?;
    if updates.len() > 1
        || updates
            .iter()
            .map(|update| update.character_id.as_str())
            .collect::<BTreeSet<_>>()
            .len()
            != updates.len()
    {
        return Err(CoreDomainRepositoryError::Integrity(
            "combat_health_replay_scope",
        ));
    }
    if let Some(changes) = changes {
        for update in updates {
            let change = changes
                .iter()
                .find(|change| change.character_id == update.character_id)
                .ok_or(CoreDomainRepositoryError::Integrity(
                    "combat_health_replay_change",
                ))?;
            if update.hp_before != change.hp_before
                || update.hp_after != change.hp_after
                || update.condition_before != change.condition_before
                || update.condition_after != change.condition_after
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "combat_health_replay_change",
                ));
            }
        }
    }
    for update in updates {
        let source_sheet_version = i64::try_from(update.source_sheet_version).map_err(|_| {
            CoreDomainRepositoryError::Integrity("combat_health_replay_sheet_version")
        })?;
        let source_character_version =
            i64::try_from(update.source_character_version).map_err(|_| {
                CoreDomainRepositoryError::Integrity("combat_health_replay_character_version")
            })?;
        let new_sheet_version =
            source_sheet_version
                .checked_add(1)
                .ok_or(CoreDomainRepositoryError::Integrity(
                    "combat_health_replay_sheet_version",
                ))?;
        if combat_health_sheet_version_id(combat_id, &update.character_id, combat_version)?
            != update.new_sheet_version_id
        {
            return Err(CoreDomainRepositoryError::Integrity(
                "combat_health_replay_identity",
            ));
        }
        let participant =
            participants
                .get(&update.character_id)
                .ok_or(CoreDomainRepositoryError::Integrity(
                    "combat_health_replay_participant",
                ))?;
        if participant.get("current_hp").and_then(Value::as_u64) != Some(u64::from(update.hp_after))
            || participant.get("condition").and_then(Value::as_str)
                != Some(update.condition_after.as_str())
        {
            return Err(CoreDomainRepositoryError::Integrity(
                "combat_health_replay_participant",
            ));
        }
        let targets_match: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1
                  FROM public.event_store AS event
                 WHERE event.sequence = $1
                   AND event.event_type = 'CombatStateRecorded'
                   AND event.campaign_id = $2
                   AND event.integrity_status = 'verified_hmac'
                   AND event.request_hash_source = 'formal_commit'
                   AND event.event_integrity_hash IS NOT NULL
                   AND EXISTS (
                        SELECT 1
                          FROM jsonb_array_elements(
                               event.projection_targets
                          ) AS target
                         WHERE target ->> 'relation' =
                               'public.character_sheet_versions'
                           AND target ->> 'row_id' = $3
                   )
                   AND EXISTS (
                        SELECT 1
                          FROM jsonb_array_elements(
                               event.projection_targets
                          ) AS target
                         WHERE target ->> 'relation' =
                               'public.characters'
                           AND target ->> 'row_id' = $4
                   )
            )
            "#,
        )
        .bind(replay.sequence)
        .bind(&replay.campaign_id)
        .bind(&update.new_sheet_version_id)
        .bind(&update.character_id)
        .fetch_one(&mut **transaction)
        .await
        .map_err(database_error("verify_combat_health_replay_targets"))?;
        if !targets_match {
            return Err(CoreDomainRepositoryError::Integrity(
                "combat_health_replay_targets",
            ));
        }
        let source = sqlx::query(
            r#"
            SELECT sheet.sheet_version_id, sheet.sheet_json, sheet.locked,
                   sheet.visibility_label::TEXT AS sheet_visibility_label,
                   sheet.visibility_subject AS sheet_visibility_subject,
                   character.current_sheet_version,
                   character.version AS character_version,
                   character.visibility_label::TEXT
                       AS character_visibility_label,
                   character.visibility_subject
                       AS character_visibility_subject,
                   character.last_event_sequence AS character_event_sequence
              FROM public.character_sheet_versions AS sheet
              JOIN public.characters AS character
                ON character.character_id = sheet.character_id
               AND character.campaign_id = sheet.campaign_id
             WHERE sheet.character_id = $1
               AND sheet.campaign_id = $2
               AND sheet.version = $3
             FOR UPDATE OF character, sheet
            "#,
        )
        .bind(&update.character_id)
        .bind(&replay.campaign_id)
        .bind(source_sheet_version)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(database_error("load_combat_health_replay_source"))?
        .ok_or(CoreDomainRepositoryError::Integrity(
            "combat_health_replay_source_missing",
        ))?;
        if !source.get::<bool, _>("locked")
            || source.get::<String, _>("sheet_visibility_label")
                != source.get::<String, _>("character_visibility_label")
            || source.get::<String, _>("sheet_visibility_subject")
                != source.get::<String, _>("character_visibility_subject")
        {
            return Err(CoreDomainRepositoryError::Integrity(
                "combat_health_replay_source_envelope",
            ));
        }
        let current_sheet_version: i64 = source.get("current_sheet_version");
        let character_version: i64 = source.get("character_version");
        let character_event_sequence: i64 = source.get("character_event_sequence");
        let character_at_source = current_sheet_version == source_sheet_version
            && character_version == source_character_version
            && character_event_sequence < replay.sequence;
        let character_at_event = current_sheet_version == new_sheet_version
            && character_version == source_character_version + 1
            && character_event_sequence == replay.sequence;
        let character_after_event = current_sheet_version >= new_sheet_version
            && character_version > source_character_version
            && character_event_sequence > replay.sequence;
        if !character_at_source && !character_at_event && !character_after_event {
            return Err(CoreDomainRepositoryError::Integrity(
                "combat_health_replay_character_position",
            ));
        }
        let mut sheet_json: Value = source.get("sheet_json");
        let profile = sheet_json
            .get_mut("combat_profile")
            .and_then(Value::as_object_mut)
            .ok_or(CoreDomainRepositoryError::Integrity(
                "combat_health_replay_profile",
            ))?;
        profile.insert("current_hp".to_owned(), Value::from(update.hp_after));
        profile.insert(
            "condition".to_owned(),
            Value::String(update.condition_after.clone()),
        );
        sqlx::query(
            r#"
            INSERT INTO public.character_sheet_versions (
                sheet_version_id, character_id, version, sheet_json, locked,
                visibility_label, visibility_subject,
                provenance_kind, provenance_reference, provenance_recorded_by,
                campaign_id, last_event_sequence
            ) VALUES (
                $1, $2, $3, $4, TRUE, $5, $6, $7, $8, $9, $10, $11
            )
            ON CONFLICT (sheet_version_id) DO NOTHING
            "#,
        )
        .bind(&update.new_sheet_version_id)
        .bind(&update.character_id)
        .bind(new_sheet_version)
        .bind(sqlx::types::Json(&sheet_json))
        .bind(source.get::<String, _>("sheet_visibility_label"))
        .bind(source.get::<String, _>("sheet_visibility_subject"))
        .bind(&replay.provenance_kind)
        .bind(&replay.provenance_reference)
        .bind(&replay.provenance_recorded_by)
        .bind(&replay.campaign_id)
        .bind(replay.sequence)
        .execute(&mut **transaction)
        .await
        .map_err(database_error("replay_combat_health_sheet"))?;
        if character_at_source {
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
                "#,
            )
            .bind(new_sheet_version)
            .bind(source_character_version + 1)
            .bind(&replay.provenance_kind)
            .bind(&replay.provenance_reference)
            .bind(&replay.provenance_recorded_by)
            .bind(replay.sequence)
            .bind(&update.character_id)
            .bind(&replay.campaign_id)
            .bind(source_sheet_version)
            .bind(source_character_version)
            .execute(&mut **transaction)
            .await
            .map_err(database_error("replay_combat_health_character"))?;
            if advanced.rows_affected() != 1 {
                return Err(CoreDomainRepositoryError::Integrity(
                    "combat_health_replay_character_conflict",
                ));
            }
        }
        let matches: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1
                  FROM public.character_sheet_versions
                 WHERE sheet_version_id = $1
                   AND character_id = $2
                   AND version = $3
                   AND sheet_json = $4
                   AND locked
                   AND campaign_id = $5
                   AND last_event_sequence = $6
            )
            "#,
        )
        .bind(&update.new_sheet_version_id)
        .bind(&update.character_id)
        .bind(new_sheet_version)
        .bind(sqlx::types::Json(&sheet_json))
        .bind(&replay.campaign_id)
        .bind(replay.sequence)
        .fetch_one(&mut **transaction)
        .await
        .map_err(database_error("verify_replayed_combat_health_sheet"))?;
        if !matches {
            return Err(CoreDomainRepositoryError::Integrity(
                "combat_health_replay_projection_mismatch",
            ));
        }
    }
    Ok(())
}
