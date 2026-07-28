impl CoreDomainRepository {
    #[allow(clippy::too_many_arguments)]
    async fn resolve_existing_growth_commit(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        metadata: &CoreCommandMetadata,
        request: &RecordGrowthRequest,
        improvement_check_roll: u8,
        increase_roll: Option<u8>,
        server_roll_id: &String,
        increase_roll_id: &Option<String>,
        roll_consumptions: &[GameplayRollConsumption],
    ) -> Result<Option<PersistedCommit>, CoreDomainRepositoryError> {
        if let Some(existing_sequence) = sqlx::query_scalar::<_, i64>(
            "SELECT last_event_sequence FROM public.growth_events WHERE growth_event_id = $1",
        )
        .bind(&request.growth_event_id)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(database_error("load_existing_growth"))?
        {
            if !self
                .projection_matches_command(existing_sequence, metadata)
                .await?
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "growth_event_identity_conflict",
                ));
            }
            let existing_event = self
                .load_idempotent_core_event(
                    &request.campaign_id,
                    &request.growth_event_id,
                    metadata,
                    "CharacterGrowthApplied",
                )
                .await?
                .ok_or(CoreDomainRepositoryError::Integrity(
                    "idempotent_growth_event_missing",
                ))?;
            if !matches!(
                &existing_event,
                CoreDomainEvent::CharacterGrowthApplied {
                    growth_event_id,
                    campaign_id,
                    session_id,
                    ending_event_id,
                    character_id,
                    source_sheet_version_id,
                    new_sheet_version_id,
                    skill_name,
                    improvement_check_roll: event_check_roll,
                    increase_roll: event_increase_roll,
                    server_roll_id: event_server_roll_id,
                    increase_roll_id: event_increase_roll_id,
                    ..
                } if growth_event_id == &request.growth_event_id
                    && campaign_id == &request.campaign_id
                    && session_id == &request.session_id
                    && ending_event_id == &request.ending_event_id
                    && character_id == &request.character_id
                    && source_sheet_version_id == &request.source_sheet_version_id
                    && new_sheet_version_id == &request.new_sheet_version_id
                    && skill_name == request.skill_name.trim()
                    && event_check_roll == &improvement_check_roll
                    && event_increase_roll == &increase_roll
                    && event_server_roll_id == server_roll_id
                    && event_increase_roll_id == increase_roll_id
            ) {
                return Err(CoreDomainRepositoryError::Integrity(
                    "idempotent_growth_request_conflict",
                ));
            }
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
                    &existing_event,
                    projection_targets,
                    "GROWTH",
                    roll_consumptions,
                )
                .await?;
            return Ok(Some(persisted));
        }
        Ok(None)
    }
}

#[derive(Clone, Copy)]
struct GrowthProjectionContext<'a> {
    metadata: &'a CoreCommandMetadata,
    request: &'a RecordGrowthRequest,
    new_sheet_version: i64,
    sheet_json: &'a Value,
    last_event_sequence: i64,
    source_version: i64,
    character_version: i64,
    skill_before: u8,
    improvement_check_roll: u8,
    increase_roll: Option<u8>,
    skill_after: u8,
    server_roll_id: &'a String,
    increase_roll_id: &'a Option<String>,
}

async fn project_growth_rows(
    transaction: &mut Transaction<'_, Postgres>,
    context: &GrowthProjectionContext<'_>,
) -> Result<(), CoreDomainRepositoryError> {
    let &GrowthProjectionContext {
        metadata,
        request,
        new_sheet_version,
        sheet_json,
        last_event_sequence,
        source_version,
        character_version,
        skill_before,
        improvement_check_roll,
        increase_roll,
        skill_after,
        server_roll_id,
        increase_roll_id,
    } = context;
    let inserted_sheet = sqlx::query(
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
    .bind(&request.new_sheet_version_id)
    .bind(&request.character_id)
    .bind(new_sheet_version)
    .bind(sqlx::types::Json(sheet_json))
    .bind(&metadata.visibility_label)
    .bind(&metadata.visibility_subject)
    .bind(&metadata.provenance_kind)
    .bind(&metadata.provenance_reference)
    .bind(&metadata.provenance_recorded_by)
    .bind(&request.campaign_id)
    .bind(last_event_sequence)
    .execute(&mut **transaction)
    .await
    .map_err(database_error("project_growth_sheet"))?;
    if inserted_sheet.rows_affected() != 1 {
        return Err(CoreDomainRepositoryError::Integrity(
            "growth_sheet_identity_conflict",
        ));
    }
    let advanced_character = sqlx::query(
        r#"
        UPDATE public.characters
           SET current_sheet_version = $1,
               version = version + 1,
               visibility_label = $2,
               visibility_subject = $3,
               provenance_kind = $4,
               provenance_reference = $5,
               provenance_recorded_by = $6,
               last_event_sequence = $7
         WHERE character_id = $8
           AND campaign_id = $9
           AND current_sheet_version = $10
           AND version = $11
        "#,
    )
    .bind(new_sheet_version)
    .bind(&metadata.visibility_label)
    .bind(&metadata.visibility_subject)
    .bind(&metadata.provenance_kind)
    .bind(&metadata.provenance_reference)
    .bind(&metadata.provenance_recorded_by)
    .bind(last_event_sequence)
    .bind(&request.character_id)
    .bind(&request.campaign_id)
    .bind(source_version)
    .bind(character_version)
    .execute(&mut **transaction)
    .await
    .map_err(database_error("project_growth_character"))?;
    if advanced_character.rows_affected() != 1 {
        return Err(CoreDomainRepositoryError::Integrity(
            "growth_character_projection_conflict",
        ));
    }
    let inserted_growth = sqlx::query(
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
    .bind(&request.growth_event_id)
    .bind(&request.campaign_id)
    .bind(&request.session_id)
    .bind(&request.ending_event_id)
    .bind(&request.character_id)
    .bind(&request.source_sheet_version_id)
    .bind(&request.new_sheet_version_id)
    .bind(request.skill_name.trim())
    .bind(i16::from(skill_before))
    .bind(i16::from(improvement_check_roll))
    .bind(increase_roll.map(i16::from))
    .bind(i16::from(skill_after))
    .bind(server_roll_id)
    .bind(increase_roll_id.as_deref())
    .bind(&metadata.visibility_label)
    .bind(&metadata.visibility_subject)
    .bind(&metadata.provenance_kind)
    .bind(&metadata.provenance_reference)
    .bind(&metadata.provenance_recorded_by)
    .bind(last_event_sequence)
    .execute(&mut **transaction)
    .await
    .map_err(database_error("project_growth_event"))?;
    if inserted_growth.rows_affected() != 1 {
        return Err(CoreDomainRepositoryError::Integrity(
            "growth_event_identity_conflict",
        ));
    }
    Ok(())
}
