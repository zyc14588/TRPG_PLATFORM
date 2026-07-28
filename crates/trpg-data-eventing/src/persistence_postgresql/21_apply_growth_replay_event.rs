
#[allow(clippy::too_many_arguments)]
async fn apply_growth_replay_event(
    transaction: &mut Transaction<'_, Postgres>,
    replay: &CanonicalReplayEvent,
    growth_event_id: &str,
    campaign_id: &str,
    session_id: &str,
    ending_event_id: &str,
    character_id: &str,
    source_sheet_version_id: &str,
    new_sheet_version_id: &str,
    skill_name: &str,
    skill_before: u8,
    improvement_check_roll: u8,
    increase_roll: Option<u8>,
    skill_after: u8,
    server_roll_id: &str,
    increase_roll_id: Option<&str>,
) -> Result<(), CoreDomainRepositoryError> {
    let qualifies = skill_before < 99
        && (improvement_check_roll > skill_before || improvement_check_roll >= 96);
    let outcome_valid = (1..=100).contains(&improvement_check_roll)
        && match increase_roll {
            Some(increase) => {
                qualifies
                    && (1..=10).contains(&increase)
                    && skill_after == skill_before.saturating_add(increase).min(99)
            }
            None => !qualifies && skill_after == skill_before,
        };
    let evidence_ids_valid = server_roll_id.len() <= 128
        && match (increase_roll, increase_roll_id) {
            (Some(_), Some(increase_id)) => {
                !increase_id.trim().is_empty()
                    && increase_id.len() <= 128
                    && increase_id != server_roll_id
            }
            (None, None) => true,
            _ => false,
        };
    if campaign_id != replay.campaign_id
        || skill_name.trim().is_empty()
        || skill_name.len() > 128
        || server_roll_id.trim().is_empty()
        || !evidence_ids_valid
        || !outcome_valid
    {
        return Err(CoreDomainRepositoryError::Integrity(
            "growth_replay_event_shape",
        ));
    }
    let roll_consumptions = growth_gameplay_roll_consumptions(server_roll_id, increase_roll_id)?;
    let existing_growth: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM public.growth_events WHERE growth_event_id = $1)",
    )
    .bind(growth_event_id)
    .fetch_one(&mut **transaction)
    .await
    .map_err(database_error("load_growth_replay_projection"))?;
    let context = GrowthReplayContext {
        replay,
        growth_event_id,
        campaign_id,
        session_id,
        ending_event_id,
        character_id,
        source_sheet_version_id,
        new_sheet_version_id,
        skill_name,
        skill_before,
        improvement_check_roll,
        increase_roll,
        skill_after,
        server_roll_id,
        increase_roll_id,
    };
    if !existing_growth {
        materialize_growth_replay(transaction, &context).await?;
    }
    project_gameplay_roll_consumptions(
        transaction,
        &roll_consumptions,
        campaign_id,
        "GROWTH",
        growth_event_id,
        &replay.visibility_label,
        &replay.visibility_subject,
        &replay.provenance_kind,
        &replay.provenance_reference,
        &replay.provenance_recorded_by,
        replay.sequence,
    )
    .await?;
    let matches: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1
              FROM public.growth_events AS growth
              JOIN public.characters AS character
                ON character.character_id = growth.character_id
               AND character.campaign_id = growth.campaign_id
              JOIN public.character_sheet_versions AS sheet
                ON sheet.sheet_version_id = growth.new_sheet_version_id
               AND sheet.character_id = growth.character_id
             WHERE growth.growth_event_id = $1
               AND growth.campaign_id = $2 AND growth.session_id = $3
               AND growth.ending_event_id = $4 AND growth.character_id = $5
               AND growth.source_sheet_version_id = $6
               AND growth.new_sheet_version_id = $7
               AND growth.skill_name = $8 AND growth.skill_before = $9
               AND growth.improvement_check_roll = $10
               AND growth.increase_roll IS NOT DISTINCT FROM $11
               AND growth.skill_after = $12 AND growth.server_roll_id = $13
               AND growth.increase_roll_id IS NOT DISTINCT FROM $14
               AND growth.random_source = 'SERVER_OS_CSPRNG'
               AND growth.last_event_sequence = $15
               AND character.current_sheet_version >= sheet.version
               AND character.last_event_sequence >= $15
               AND sheet.sheet_json -> 'skills' ->> $8 = $12::TEXT
               AND sheet.last_event_sequence = $15
        )
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
    .bind(replay.sequence)
    .fetch_one(&mut **transaction)
    .await
    .map_err(database_error("verify_replayed_growth"))?;
    if !matches {
        return Err(CoreDomainRepositoryError::Integrity(
            "growth_replay_projection_mismatch",
        ));
    }
    Ok(())
}
