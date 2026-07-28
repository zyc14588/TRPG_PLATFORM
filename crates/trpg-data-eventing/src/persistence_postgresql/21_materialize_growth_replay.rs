#[derive(Clone, Copy)]
struct GrowthReplayContext<'a> {
    replay: &'a CanonicalReplayEvent,
    growth_event_id: &'a str,
    campaign_id: &'a str,
    session_id: &'a str,
    ending_event_id: &'a str,
    character_id: &'a str,
    source_sheet_version_id: &'a str,
    new_sheet_version_id: &'a str,
    skill_name: &'a str,
    skill_before: u8,
    improvement_check_roll: u8,
    increase_roll: Option<u8>,
    skill_after: u8,
    server_roll_id: &'a str,
    increase_roll_id: Option<&'a str>,
}

async fn materialize_growth_replay(
    transaction: &mut Transaction<'_, Postgres>,
    context: &GrowthReplayContext<'_>,
) -> Result<(), CoreDomainRepositoryError> {
    let &GrowthReplayContext { replay, growth_event_id, campaign_id, session_id, ending_event_id, character_id, source_sheet_version_id, new_sheet_version_id, skill_name, skill_before, improvement_check_roll, increase_roll, skill_after, server_roll_id, increase_roll_id } = context;
    let source = sqlx::query(
        r#"
        SELECT character.current_sheet_version,
               character.version AS character_version,
               character.last_event_sequence AS character_event_sequence,
               sheet.version AS sheet_version,
               sheet.sheet_json
          FROM public.characters AS character
          JOIN public.character_sheet_versions AS sheet
            ON sheet.character_id = character.character_id
           AND sheet.sheet_version_id = $1
         WHERE character.character_id = $2
           AND character.campaign_id = $3
           AND sheet.campaign_id = $3
           AND sheet.locked
        "#,
    )
    .bind(source_sheet_version_id)
    .bind(character_id)
    .bind(campaign_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(database_error("load_growth_replay_source"))?
    .ok_or(CoreDomainRepositoryError::Integrity(
        "growth_replay_source_missing",
    ))?;
    let source_version: i64 = source.get("sheet_version");
    let character_sheet_version: i64 = source.get("current_sheet_version");
    let character_version: i64 = source.get("character_version");
    let character_event_sequence: i64 = source.get("character_event_sequence");
    let new_sheet_version =
        source_version
            .checked_add(1)
            .ok_or(CoreDomainRepositoryError::Integrity(
                "growth_replay_sheet_version",
            ))?;
    let character_at_source =
        character_sheet_version == source_version && character_event_sequence < replay.sequence;
    let character_at_growth = character_sheet_version == new_sheet_version
        && character_event_sequence == replay.sequence;
    let character_after_growth = character_sheet_version >= new_sheet_version
        && character_event_sequence > replay.sequence;
    if !character_at_source && !character_at_growth && !character_after_growth {
        return Err(CoreDomainRepositoryError::Integrity(
            "growth_replay_character_position",
        ));
    }
    let expected_character_version: i64 = sqlx::query_scalar(
        r#"
        SELECT count(*)
          FROM public.event_store AS event
         WHERE event.campaign_id = $1
           AND event.sequence <= $2
           AND event.integrity_status = 'verified_hmac'
           AND event.request_hash_source = 'formal_commit'
           AND event.event_integrity_hash IS NOT NULL
           AND EXISTS (
                SELECT 1
                  FROM jsonb_array_elements(
                       event.projection_targets
                  ) AS projection_target
                 WHERE projection_target ->> 'relation' =
                       'public.characters'
                   AND projection_target ->> 'row_id' = $3
           )
        "#,
    )
    .bind(campaign_id)
    .bind(if character_after_growth {
        character_event_sequence
    } else {
        replay.sequence
    })
    .bind(character_id)
    .fetch_one(&mut **transaction)
    .await
    .map_err(database_error("count_growth_replay_character_version"))?;
    if (character_at_source && character_version + 1 != expected_character_version)
        || (!character_at_source && character_version != expected_character_version)
    {
        return Err(CoreDomainRepositoryError::Integrity(
            "growth_replay_character_version",
        ));
    }
    if character_after_growth {
        let later_character_is_canonical: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1
                  FROM public.characters AS character
                  JOIN public.event_store AS event
                    ON event.sequence = character.last_event_sequence
                   AND event.campaign_id = character.campaign_id
                 WHERE character.character_id = $1
                   AND character.campaign_id = $2
                   AND character.last_event_sequence = $3
                   AND character.version = $4
                   AND event.integrity_status = 'verified_hmac'
                   AND event.request_hash_source = 'formal_commit'
                   AND event.event_integrity_hash IS NOT NULL
                   AND (
                        event.event_type = 'CombatStateRecorded'
                        OR event.visibility_label =
                           character.visibility_label::TEXT
                           AND event.visibility_subject =
                               character.visibility_subject
                   )
                   AND event.fact_provenance_kind =
                       character.provenance_kind::TEXT
                   AND event.fact_provenance_reference =
                       character.provenance_reference
                   AND event.fact_recorded_by =
                       character.provenance_recorded_by
                   AND EXISTS (
                        SELECT 1
                          FROM jsonb_array_elements(
                               event.projection_targets
                          ) AS projection_target
                         WHERE projection_target ->> 'relation' =
                               'public.characters'
                           AND projection_target ->> 'row_id' = $1
                   )
            )
            "#,
        )
        .bind(character_id)
        .bind(campaign_id)
        .bind(character_event_sequence)
        .bind(character_version)
        .fetch_one(&mut **transaction)
        .await
        .map_err(database_error("verify_later_growth_character"))?;
        if !later_character_is_canonical {
            return Err(CoreDomainRepositoryError::Integrity(
                "growth_replay_later_character_mismatch",
            ));
        }
    }
    let mut sheet_json: Value = source.get("sheet_json");
    if sheet_json
        .get("skills")
        .and_then(Value::as_object)
        .and_then(|skills| skills.get(skill_name))
        .and_then(Value::as_u64)
        != Some(u64::from(skill_before))
    {
        return Err(CoreDomainRepositoryError::Integrity(
            "growth_replay_skill_source",
        ));
    }
    sheet_json
        .get_mut("skills")
        .and_then(Value::as_object_mut)
        .ok_or(CoreDomainRepositoryError::Integrity(
            "growth_replay_skills_missing",
        ))?
        .insert(skill_name.to_owned(), Value::from(skill_after));
    sync_combat_skill_target(&mut sheet_json, skill_name, skill_before, skill_after)?;
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
    .bind(new_sheet_version_id)
    .bind(character_id)
    .bind(new_sheet_version)
    .bind(sqlx::types::Json(&sheet_json))
    .bind(&replay.visibility_label)
    .bind(&replay.visibility_subject)
    .bind(&replay.provenance_kind)
    .bind(&replay.provenance_reference)
    .bind(&replay.provenance_recorded_by)
    .bind(campaign_id)
    .bind(replay.sequence)
    .execute(&mut **transaction)
    .await
    .map_err(database_error("replay_growth_sheet"))?;
    if character_at_source {
        let advanced = sqlx::query(
            r#"
            UPDATE public.characters
               SET current_sheet_version = $1,
                   version = $2,
                   visibility_label = $3,
                   visibility_subject = $4,
                   provenance_kind = $5,
                   provenance_reference = $6,
                   provenance_recorded_by = $7,
                   last_event_sequence = $8
             WHERE character_id = $9 AND campaign_id = $10
               AND current_sheet_version = $11 AND version = $12
               AND last_event_sequence = $13
            "#,
        )
        .bind(new_sheet_version)
        .bind(expected_character_version)
        .bind(&replay.visibility_label)
        .bind(&replay.visibility_subject)
        .bind(&replay.provenance_kind)
        .bind(&replay.provenance_reference)
        .bind(&replay.provenance_recorded_by)
        .bind(replay.sequence)
        .bind(character_id)
        .bind(campaign_id)
        .bind(source_version)
        .bind(character_version)
        .bind(character_event_sequence)
        .execute(&mut **transaction)
        .await
        .map_err(database_error("advance_replayed_growth_character"))?;
        if advanced.rows_affected() != 1 {
            return Err(CoreDomainRepositoryError::Integrity(
                "growth_replay_character_conflict",
            ));
        }
    } else if character_at_growth {
        let character_matches: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1
                  FROM public.characters
                 WHERE character_id = $1 AND campaign_id = $2
                   AND current_sheet_version = $3 AND version = $4
                   AND visibility_label::TEXT = $5
                   AND visibility_subject = $6
                   AND provenance_kind::TEXT = $7
                   AND provenance_reference = $8
                   AND provenance_recorded_by = $9
                   AND last_event_sequence = $10
            )
            "#,
        )
        .bind(character_id)
        .bind(campaign_id)
        .bind(new_sheet_version)
        .bind(expected_character_version)
        .bind(&replay.visibility_label)
        .bind(&replay.visibility_subject)
        .bind(&replay.provenance_kind)
        .bind(&replay.provenance_reference)
        .bind(&replay.provenance_recorded_by)
        .bind(replay.sequence)
        .fetch_one(&mut **transaction)
        .await
        .map_err(database_error("verify_current_growth_character"))?;
        if !character_matches {
            sqlx::query(
                "SELECT set_config( \
                     'trpg.p08_projection_rebuild', \
                     'character_growth', \
                     TRUE \
                 )",
            )
            .execute(&mut **transaction)
            .await
            .map_err(database_error("set_growth_replay_repair_scope"))?;
            let repaired = sqlx::query(
                r#"
                UPDATE public.characters
                   SET current_sheet_version = $1,
                       version = $2,
                       visibility_label = $3,
                       visibility_subject = $4,
                       provenance_kind = $5,
                       provenance_reference = $6,
                       provenance_recorded_by = $7,
                       last_event_sequence = $8
                 WHERE character_id = $9 AND campaign_id = $10
                   AND current_sheet_version = $1
                   AND last_event_sequence = $8
                "#,
            )
            .bind(new_sheet_version)
            .bind(expected_character_version)
            .bind(&replay.visibility_label)
            .bind(&replay.visibility_subject)
            .bind(&replay.provenance_kind)
            .bind(&replay.provenance_reference)
            .bind(&replay.provenance_recorded_by)
            .bind(replay.sequence)
            .bind(character_id)
            .bind(campaign_id)
            .execute(&mut **transaction)
            .await
            .map_err(database_error("repair_replayed_growth_character"))?;
            if repaired.rows_affected() != 1 {
                return Err(CoreDomainRepositoryError::Integrity(
                    "growth_replay_character_conflict",
                ));
            }
        }
    }
    sqlx::query(
        r#"
        INSERT INTO public.growth_events (
            growth_event_id, campaign_id, session_id, ending_event_id,
            character_id, source_sheet_version_id, new_sheet_version_id,
            skill_name, skill_before, improvement_check_roll,
            increase_roll, skill_after, server_roll_id, increase_roll_id, random_source,
            version, visibility_label, visibility_subject,
            provenance_kind, provenance_reference, provenance_recorded_by,
            last_event_sequence
        ) VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
            $11, $12, $13, $14, 'SERVER_OS_CSPRNG', 1, $15, $16,
            $17, $18, $19, $20
        )
        ON CONFLICT (growth_event_id) DO NOTHING
        "#,
    )
    .bind(growth_event_id)
    .bind(campaign_id)
    .bind(session_id)
    .bind(ending_event_id)
    .bind(character_id)
    .bind(source_sheet_version_id)
    .bind(new_sheet_version_id)
    .bind(skill_name)
    .bind(i16::from(skill_before))
    .bind(i16::from(improvement_check_roll))
    .bind(increase_roll.map(i16::from))
    .bind(i16::from(skill_after))
    .bind(server_roll_id)
    .bind(increase_roll_id)
    .bind(&replay.visibility_label)
    .bind(&replay.visibility_subject)
    .bind(&replay.provenance_kind)
    .bind(&replay.provenance_reference)
    .bind(&replay.provenance_recorded_by)
    .bind(replay.sequence)
    .execute(&mut **transaction)
    .await
    .map_err(database_error("replay_growth_event"))?;
    Ok(())
}
