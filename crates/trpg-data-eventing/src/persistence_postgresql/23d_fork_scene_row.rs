async fn apply_fork_scene_row(
    transaction: &mut Transaction<'_, Postgres>,
    replay: &CanonicalReplayEvent,
    _fork_id: &String,
    child_campaign_id: &String,
    row: CampaignForkMaterializedRow,
) -> Result<(), CoreDomainRepositoryError> {
    let CampaignForkMaterializedRow::Scene {
        scene_id,
        session_id,
        scenario_id,
        room_id,
        scene_key,
        name,
        state,
        visibility_label,
        visibility_subject,
    } = row else { unreachable!("expected fork Scene row"); };
    if !matches!(state.as_str(), "READY" | "ACTIVE" | "CLOSED") {
        return Err(CoreDomainRepositoryError::Integrity("fork_scene_state"));
    }
    sqlx::query(
        r#"
        INSERT INTO public.scenes (
            scene_id, campaign_id, session_id, scenario_id, room_id,
            scene_key, name, state, version,
            visibility_label, visibility_subject,
            provenance_kind, provenance_reference,
            provenance_recorded_by, last_event_sequence
        ) VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, 1,
            $9, $10, $11, $12, $13, $14
        )
        ON CONFLICT (scene_id) DO NOTHING
        "#,
    )
    .bind(&scene_id)
    .bind(child_campaign_id)
    .bind(&session_id)
    .bind(&scenario_id)
    .bind(&room_id)
    .bind(&scene_key)
    .bind(&name)
    .bind(&state)
    .bind(&visibility_label)
    .bind(&visibility_subject)
    .bind(&replay.provenance_kind)
    .bind(&replay.provenance_reference)
    .bind(&replay.provenance_recorded_by)
    .bind(replay.sequence)
    .execute(&mut **transaction)
    .await
    .map_err(database_error("replay_fork_scene"))?;
    let matches: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM public.scenes
             WHERE scene_id = $1 AND campaign_id = $2
               AND session_id = $3 AND scenario_id = $4
               AND room_id = $5 AND scene_key = $6
               AND name = $7 AND state = $8
               AND visibility_label::TEXT = $9
               AND visibility_subject = $10
               AND last_event_sequence = $11
        )
        "#,
    )
    .bind(&scene_id)
    .bind(child_campaign_id)
    .bind(&session_id)
    .bind(&scenario_id)
    .bind(&room_id)
    .bind(&scene_key)
    .bind(&name)
    .bind(&state)
    .bind(&visibility_label)
    .bind(&visibility_subject)
    .bind(replay.sequence)
    .fetch_one(&mut **transaction)
    .await
    .map_err(database_error("verify_replayed_fork_scene"))?;
    if !matches {
        return Err(CoreDomainRepositoryError::Integrity(
            "fork_scene_identity_conflict",
        ));
    }
    Ok(())
}
