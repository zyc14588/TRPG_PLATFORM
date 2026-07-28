async fn apply_fork_combat_row(
    transaction: &mut Transaction<'_, Postgres>,
    replay: &CanonicalReplayEvent,
    _fork_id: &String,
    child_campaign_id: &String,
    row: CampaignForkMaterializedRow,
) -> Result<(), CoreDomainRepositoryError> {
    let CampaignForkMaterializedRow::Combat {
        combat_id,
        session_id,
        status,
        round,
        current_turn_index,
        state_json,
        visibility_label,
        visibility_subject,
    } = row else { unreachable!("expected fork Combat row"); };
    let inspected = inspect_combat_state(&state_json).map_err(|_| {
        CoreDomainRepositoryError::Integrity("fork_combat_state_json")
    })?;
    if inspected.combat_id() != combat_id
        || inspected.status() != status
        || u64::from(inspected.round()) != round
        || u64::try_from(inspected.current_turn_index()).ok()
            != Some(current_turn_index)
        || inspected.version() != 1
    {
        return Err(CoreDomainRepositoryError::Integrity(
            "fork_combat_state_shape",
        ));
    }
    let round = i64::try_from(round).map_err(|_| {
        CoreDomainRepositoryError::Integrity("fork_combat_round")
    })?;
    let current_turn_index =
        i64::try_from(current_turn_index).map_err(|_| {
            CoreDomainRepositoryError::Integrity("fork_combat_turn")
        })?;
    sqlx::query(
        r#"
        INSERT INTO public.combat_states (
            combat_id, campaign_id, session_id, status,
            round, current_turn_index, state_json, version,
            visibility_label, visibility_subject,
            provenance_kind, provenance_reference,
            provenance_recorded_by, last_event_sequence
        ) VALUES (
            $1, $2, $3, $4, $5, $6, $7::JSONB, 1,
            $8, $9, $10, $11, $12, $13
        )
        ON CONFLICT (combat_id) DO NOTHING
        "#,
    )
    .bind(&combat_id)
    .bind(child_campaign_id)
    .bind(&session_id)
    .bind(&status)
    .bind(round)
    .bind(current_turn_index)
    .bind(&state_json)
    .bind(&visibility_label)
    .bind(&visibility_subject)
    .bind(&replay.provenance_kind)
    .bind(&replay.provenance_reference)
    .bind(&replay.provenance_recorded_by)
    .bind(replay.sequence)
    .execute(&mut **transaction)
    .await
    .map_err(database_error("replay_fork_combat"))?;
    let matches: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM public.combat_states
             WHERE combat_id = $1
               AND campaign_id = $2
               AND session_id = $3
               AND status = $4
               AND round = $5
               AND current_turn_index = $6
               AND state_json = $7::JSONB
               AND version = 1
               AND visibility_label::TEXT = $8
               AND visibility_subject = $9
               AND last_event_sequence = $10
        )
        "#,
    )
    .bind(&combat_id)
    .bind(child_campaign_id)
    .bind(&session_id)
    .bind(&status)
    .bind(round)
    .bind(current_turn_index)
    .bind(&state_json)
    .bind(&visibility_label)
    .bind(&visibility_subject)
    .bind(replay.sequence)
    .fetch_one(&mut **transaction)
    .await
    .map_err(database_error("verify_replayed_fork_combat"))?;
    if !matches {
        return Err(CoreDomainRepositoryError::Integrity(
            "fork_combat_identity_conflict",
        ));
    }
    Ok(())
}
