async fn apply_fork_character_row(
    transaction: &mut Transaction<'_, Postgres>,
    replay: &CanonicalReplayEvent,
    _fork_id: &String,
    child_campaign_id: &String,
    row: CampaignForkMaterializedRow,
) -> Result<(), CoreDomainRepositoryError> {
    let CampaignForkMaterializedRow::Character {
        character_id,
        owner_user_id,
        display_name,
        state,
        initial_version_locked,
        sheet_version_id,
        sheet_json,
        sheet_locked,
        visibility_label,
        visibility_subject,
    } = row else { unreachable!("expected fork Character row"); };
    if !matches!(state.as_str(), "DRAFT" | "SUBMITTED" | "APPROVED") {
        return Err(CoreDomainRepositoryError::Integrity(
            "fork_character_state",
        ));
    }
    sqlx::query(
        r#"
        INSERT INTO public.characters (
            character_id, campaign_id, owner_user_id, display_name,
            state, current_sheet_version, initial_version_locked,
            version, visibility_label, visibility_subject,
            provenance_kind, provenance_reference,
            provenance_recorded_by, last_event_sequence
        ) VALUES (
            $1, $2, $3, $4, $5, 1, $6, 1, $7, $8,
            $9, $10, $11, $12
        )
        ON CONFLICT (character_id) DO NOTHING
        "#,
    )
    .bind(&character_id)
    .bind(child_campaign_id)
    .bind(&owner_user_id)
    .bind(&display_name)
    .bind(&state)
    .bind(initial_version_locked)
    .bind(&visibility_label)
    .bind(&visibility_subject)
    .bind(&replay.provenance_kind)
    .bind(&replay.provenance_reference)
    .bind(&replay.provenance_recorded_by)
    .bind(replay.sequence)
    .execute(&mut **transaction)
    .await
    .map_err(database_error("replay_fork_character"))?;
    sqlx::query(
        r#"
        INSERT INTO public.character_sheet_versions (
            sheet_version_id, character_id, version, sheet_json,
            locked, visibility_label, visibility_subject,
            provenance_kind, provenance_reference,
            provenance_recorded_by, campaign_id, last_event_sequence
        ) VALUES (
            $1, $2, 1, $3::JSONB, $4, $5, $6, $7, $8, $9, $10, $11
        )
        ON CONFLICT (sheet_version_id) DO NOTHING
        "#,
    )
    .bind(&sheet_version_id)
    .bind(&character_id)
    .bind(&sheet_json)
    .bind(sheet_locked)
    .bind(&visibility_label)
    .bind(&visibility_subject)
    .bind(&replay.provenance_kind)
    .bind(&replay.provenance_reference)
    .bind(&replay.provenance_recorded_by)
    .bind(child_campaign_id)
    .bind(replay.sequence)
    .execute(&mut **transaction)
    .await
    .map_err(database_error("replay_fork_character_sheet"))?;
    let matches: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1
              FROM public.characters AS character
              JOIN public.character_sheet_versions AS sheet
                ON sheet.character_id = character.character_id
               AND sheet.version = 1
              JOIN public.character_sheet_versions
                   AS current_sheet
                ON current_sheet.character_id =
                   character.character_id
               AND current_sheet.version =
                   character.current_sheet_version
              JOIN public.event_store AS character_event
                ON character_event.sequence =
                   character.last_event_sequence
               AND character_event.campaign_id =
                   character.campaign_id
              JOIN public.event_store AS sheet_event
                ON sheet_event.sequence =
                   sheet.last_event_sequence
               AND sheet_event.campaign_id =
                   character.campaign_id
              JOIN public.event_store
                   AS current_sheet_event
                ON current_sheet_event.sequence =
                   current_sheet.last_event_sequence
               AND current_sheet_event.campaign_id =
                   character.campaign_id
             WHERE character.character_id = $1
               AND character.campaign_id = $2
               AND character.owner_user_id = $3
               AND character.display_name = $4
               AND character.last_event_sequence >= $7
               AND (
                    (
                        character.last_event_sequence = $7
                        AND character.state = $5
                        AND character.initial_version_locked = $6
                        AND character.visibility_label::TEXT = $11
                        AND character.visibility_subject = $12
                    )
                    OR (
                        character.last_event_sequence > $7
                        AND CASE $5
                            WHEN 'DRAFT' THEN
                                character.state IN (
                                    'DRAFT',
                                    'SUBMITTED',
                                    'APPROVED'
                                )
                            WHEN 'SUBMITTED' THEN
                                character.state IN (
                                    'SUBMITTED',
                                    'APPROVED'
                                )
                            WHEN 'APPROVED' THEN
                                character.state = 'APPROVED'
                            ELSE FALSE
                        END
                        AND (
                            NOT $6
                            OR character.initial_version_locked
                        )
                        AND (
                            character.state <> 'APPROVED'
                            OR character.initial_version_locked
                        )
                    )
               )
               AND character_event.integrity_status =
                   'verified_hmac'
               AND character_event.request_hash_source =
                   'formal_commit'
               AND character_event.event_integrity_hash
                   IS NOT NULL
               AND (
                    character_event.event_type =
                        'CombatStateRecorded'
                    OR character_event.visibility_label =
                       character.visibility_label::TEXT
                       AND character_event
                           .visibility_subject =
                           character.visibility_subject
               )
               AND character_event.fact_provenance_kind =
                   character.provenance_kind::TEXT
               AND character_event.fact_provenance_reference =
                   character.provenance_reference
               AND character_event.fact_recorded_by =
                   character.provenance_recorded_by
               AND EXISTS (
                    SELECT 1
                      FROM jsonb_array_elements(
                           character_event.projection_targets
                      ) AS projection_target
                     WHERE projection_target ->> 'relation' =
                           'public.characters'
                       AND projection_target ->> 'row_id' =
                           character.character_id
               )
               AND character.last_event_sequence = (
                    SELECT max(event.sequence)
                      FROM public.event_store AS event
                     WHERE event.campaign_id =
                           character.campaign_id
                       AND event.integrity_status =
                           'verified_hmac'
                       AND event.request_hash_source =
                           'formal_commit'
                       AND event.event_integrity_hash
                           IS NOT NULL
                       AND EXISTS (
                            SELECT 1
                              FROM jsonb_array_elements(
                                   event.projection_targets
                              ) AS projection_target
                             WHERE projection_target ->>
                                   'relation' =
                                   'public.characters'
                               AND projection_target ->>
                                   'row_id' =
                                   character.character_id
                       )
               )
               AND character.version = (
                    SELECT count(*)
                      FROM public.event_store AS event
                     WHERE event.campaign_id =
                           character.campaign_id
                       AND event.sequence <=
                           character.last_event_sequence
                       AND event.integrity_status =
                           'verified_hmac'
                       AND event.request_hash_source =
                           'formal_commit'
                       AND event.event_integrity_hash
                           IS NOT NULL
                       AND EXISTS (
                            SELECT 1
                              FROM jsonb_array_elements(
                                   event.projection_targets
                              ) AS projection_target
                             WHERE projection_target ->>
                                   'relation' =
                                   'public.characters'
                               AND projection_target ->>
                                   'row_id' =
                                   character.character_id
                       )
               )
               AND sheet.sheet_version_id = $8
               AND sheet.sheet_json = $9::JSONB
               AND sheet.last_event_sequence >= $7
               AND (
                    (
                        sheet.last_event_sequence = $7
                        AND sheet.locked = $10
                        AND sheet.visibility_label::TEXT = $11
                        AND sheet.visibility_subject = $12
                    )
                    OR (
                        sheet.last_event_sequence > $7
                        AND (NOT $10 OR sheet.locked)
                    )
               )
               AND sheet_event.integrity_status =
                   'verified_hmac'
               AND sheet_event.request_hash_source =
                   'formal_commit'
               AND sheet_event.event_integrity_hash
                   IS NOT NULL
               AND sheet_event.visibility_label =
                   sheet.visibility_label::TEXT
               AND sheet_event.visibility_subject =
                   sheet.visibility_subject
               AND sheet_event.fact_provenance_kind =
                   sheet.provenance_kind::TEXT
               AND sheet_event.fact_provenance_reference =
                   sheet.provenance_reference
               AND sheet_event.fact_recorded_by =
                   sheet.provenance_recorded_by
               AND EXISTS (
                    SELECT 1
                      FROM jsonb_array_elements(
                           sheet_event.projection_targets
                      ) AS projection_target
                     WHERE projection_target ->> 'relation' =
                           'public.character_sheet_versions'
                       AND projection_target ->> 'row_id' =
                           sheet.sheet_version_id
               )
               AND sheet.last_event_sequence = (
                    SELECT max(event.sequence)
                      FROM public.event_store AS event
                     WHERE event.campaign_id =
                           character.campaign_id
                       AND event.integrity_status =
                           'verified_hmac'
                       AND event.request_hash_source =
                           'formal_commit'
                       AND event.event_integrity_hash
                           IS NOT NULL
                       AND EXISTS (
                            SELECT 1
                              FROM jsonb_array_elements(
                                   event.projection_targets
                              ) AS projection_target
                             WHERE projection_target ->>
                                   'relation' =
                                   'public.character_sheet_versions'
                               AND projection_target ->>
                                   'row_id' =
                                   sheet.sheet_version_id
                       )
               )
               AND current_sheet.last_event_sequence <=
                   character.last_event_sequence
               AND current_sheet_event.integrity_status =
                   'verified_hmac'
               AND current_sheet_event.request_hash_source =
                   'formal_commit'
               AND current_sheet_event.event_integrity_hash
                   IS NOT NULL
               AND (
                    current_sheet_event.event_type =
                        'CombatStateRecorded'
                    OR current_sheet_event
                           .visibility_label =
                       current_sheet
                           .visibility_label::TEXT
                       AND current_sheet_event
                           .visibility_subject =
                           current_sheet
                               .visibility_subject
               )
               AND current_sheet_event.fact_provenance_kind =
                   current_sheet.provenance_kind::TEXT
               AND current_sheet_event
                   .fact_provenance_reference =
                   current_sheet.provenance_reference
               AND current_sheet_event.fact_recorded_by =
                   current_sheet.provenance_recorded_by
               AND EXISTS (
                    SELECT 1
                      FROM jsonb_array_elements(
                           current_sheet_event
                               .projection_targets
                      ) AS projection_target
                     WHERE projection_target ->> 'relation' =
                           'public.character_sheet_versions'
                       AND projection_target ->> 'row_id' =
                           current_sheet.sheet_version_id
               )
               AND current_sheet.last_event_sequence = (
                    SELECT max(event.sequence)
                      FROM public.event_store AS event
                     WHERE event.campaign_id =
                           character.campaign_id
                       AND event.integrity_status =
                           'verified_hmac'
                       AND event.request_hash_source =
                           'formal_commit'
                       AND event.event_integrity_hash
                           IS NOT NULL
                       AND EXISTS (
                            SELECT 1
                              FROM jsonb_array_elements(
                                   event.projection_targets
                              ) AS projection_target
                             WHERE projection_target ->>
                                   'relation' =
                                   'public.character_sheet_versions'
                               AND projection_target ->>
                                   'row_id' =
                                   current_sheet
                                       .sheet_version_id
                       )
               )
        )
        "#,
    )
    .bind(&character_id)
    .bind(child_campaign_id)
    .bind(&owner_user_id)
    .bind(&display_name)
    .bind(&state)
    .bind(initial_version_locked)
    .bind(replay.sequence)
    .bind(&sheet_version_id)
    .bind(&sheet_json)
    .bind(sheet_locked)
    .bind(&visibility_label)
    .bind(&visibility_subject)
    .fetch_one(&mut **transaction)
    .await
    .map_err(database_error("verify_replayed_fork_character"))?;
    if !matches {
        return Err(CoreDomainRepositoryError::Integrity(
            "fork_character_identity_conflict",
        ));
    }
    Ok(())
}
