async fn apply_fork_discovered_clue_row(
    transaction: &mut Transaction<'_, Postgres>,
    replay: &CanonicalReplayEvent,
    fork_id: &String,
    child_campaign_id: &String,
    row: CampaignForkMaterializedRow,
) -> Result<(), CoreDomainRepositoryError> {
    let CampaignForkMaterializedRow::DiscoveredClue {
        fork_clue_id,
        source_clue_id,
        importance,
        outcome,
        cost,
        visibility_label,
        visibility_subject,
    } = row else { unreachable!("expected fork DiscoveredClue row"); };
    if !matches!(importance.as_str(), "CORE" | "OPTIONAL")
        || !matches!(outcome.as_str(), "REVEALED" | "REVEALED_WITH_COST")
        || !matches!(visibility_label.as_str(), "public" | "party_visible")
        || visibility_subject != "not_applicable"
    {
        return Err(CoreDomainRepositoryError::Integrity("fork_clue_shape"));
    }
    sqlx::query(
        r#"
        INSERT INTO public.campaign_fork_clues (
            fork_clue_id, fork_id, campaign_id, source_clue_id,
            importance, outcome, cost, version,
            visibility_label, visibility_subject,
            provenance_kind, provenance_reference,
            provenance_recorded_by, last_event_sequence
        ) VALUES (
            $1, $2, $3, $4, $5, $6, $7, 1,
            $8, $9, $10, $11, $12, $13
        )
        ON CONFLICT (fork_clue_id) DO NOTHING
        "#,
    )
    .bind(&fork_clue_id)
    .bind(fork_id)
    .bind(child_campaign_id)
    .bind(&source_clue_id)
    .bind(&importance)
    .bind(&outcome)
    .bind(&cost)
    .bind(&visibility_label)
    .bind(&visibility_subject)
    .bind(&replay.provenance_kind)
    .bind(&replay.provenance_reference)
    .bind(&replay.provenance_recorded_by)
    .bind(replay.sequence)
    .execute(&mut **transaction)
    .await
    .map_err(database_error("replay_fork_clue"))?;
    let matches: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM public.campaign_fork_clues
             WHERE fork_clue_id = $1
               AND fork_id = $2
               AND campaign_id = $3
               AND source_clue_id = $4
               AND importance = $5
               AND outcome = $6
               AND cost IS NOT DISTINCT FROM $7
               AND visibility_label::TEXT = $8
               AND visibility_subject = $9
               AND last_event_sequence = $10
        )
        "#,
    )
    .bind(&fork_clue_id)
    .bind(fork_id)
    .bind(child_campaign_id)
    .bind(&source_clue_id)
    .bind(&importance)
    .bind(&outcome)
    .bind(&cost)
    .bind(&visibility_label)
    .bind(&visibility_subject)
    .bind(replay.sequence)
    .fetch_one(&mut **transaction)
    .await
    .map_err(database_error("verify_replayed_fork_clue"))?;
    if !matches {
        return Err(CoreDomainRepositoryError::Integrity(
            "fork_clue_identity_conflict",
        ));
    }
    Ok(())
}
