async fn apply_fork_npc_state_row(
    transaction: &mut Transaction<'_, Postgres>,
    replay: &CanonicalReplayEvent,
    fork_id: &String,
    child_campaign_id: &String,
    row: CampaignForkMaterializedRow,
) -> Result<(), CoreDomainRepositoryError> {
    let CampaignForkMaterializedRow::NpcState {
        npc_state_id,
        source_npc_id,
        state_json,
        visibility_label,
        visibility_subject,
    } = row else { unreachable!("expected fork NpcState row"); };
    let state: Value = serde_json::from_str(&state_json).map_err(|_| {
        CoreDomainRepositoryError::Integrity("fork_npc_state_json")
    })?;
    if !state.is_object()
        || !matches!(visibility_label.as_str(), "public" | "party_visible")
        || visibility_subject != "not_applicable"
    {
        return Err(CoreDomainRepositoryError::Integrity(
            "fork_npc_state_shape",
        ));
    }
    sqlx::query(
        r#"
        INSERT INTO public.campaign_fork_npc_states (
            npc_state_id, fork_id, campaign_id, source_npc_id,
            state_json, version,
            visibility_label, visibility_subject,
            provenance_kind, provenance_reference,
            provenance_recorded_by, last_event_sequence
        ) VALUES (
            $1, $2, $3, $4, $5::JSONB, 1,
            $6, $7, $8, $9, $10, $11
        )
        ON CONFLICT (npc_state_id) DO NOTHING
        "#,
    )
    .bind(&npc_state_id)
    .bind(fork_id)
    .bind(child_campaign_id)
    .bind(&source_npc_id)
    .bind(&state_json)
    .bind(&visibility_label)
    .bind(&visibility_subject)
    .bind(&replay.provenance_kind)
    .bind(&replay.provenance_reference)
    .bind(&replay.provenance_recorded_by)
    .bind(replay.sequence)
    .execute(&mut **transaction)
    .await
    .map_err(database_error("replay_fork_npc_state"))?;
    let matches: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM public.campaign_fork_npc_states
             WHERE npc_state_id = $1
               AND fork_id = $2
               AND campaign_id = $3
               AND source_npc_id = $4
               AND state_json = $5::JSONB
               AND visibility_label::TEXT = $6
               AND visibility_subject = $7
               AND last_event_sequence = $8
        )
        "#,
    )
    .bind(&npc_state_id)
    .bind(fork_id)
    .bind(child_campaign_id)
    .bind(&source_npc_id)
    .bind(&state_json)
    .bind(&visibility_label)
    .bind(&visibility_subject)
    .bind(replay.sequence)
    .fetch_one(&mut **transaction)
    .await
    .map_err(database_error("verify_replayed_fork_npc_state"))?;
    if !matches {
        return Err(CoreDomainRepositoryError::Integrity(
            "fork_npc_state_identity_conflict",
        ));
    }
    Ok(())
}
