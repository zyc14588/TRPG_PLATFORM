async fn apply_fork_scenario_row(
    transaction: &mut Transaction<'_, Postgres>,
    replay: &CanonicalReplayEvent,
    _fork_id: &String,
    child_campaign_id: &String,
    row: CampaignForkMaterializedRow,
) -> Result<(), CoreDomainRepositoryError> {
    let CampaignForkMaterializedRow::Scenario {
        scenario_id,
        ruleset_id,
        format_version,
        content_hash,
        document_json,
        visibility_label,
        visibility_subject,
    } = row else { unreachable!("expected fork Scenario row"); };
    sqlx::query(
        r#"
        INSERT INTO public.scenarios (
            scenario_id, campaign_id, ruleset_id, format_version,
            content_hash, document_json, validated, version,
            visibility_label, visibility_subject,
            provenance_kind, provenance_reference,
            provenance_recorded_by, last_event_sequence
        ) VALUES (
            $1, $2, $3, $4, $5, $6::JSONB, TRUE, 1,
            $7, $8, $9, $10, $11, $12
        )
        ON CONFLICT (scenario_id) DO NOTHING
        "#,
    )
    .bind(&scenario_id)
    .bind(child_campaign_id)
    .bind(&ruleset_id)
    .bind(&format_version)
    .bind(&content_hash)
    .bind(&document_json)
    .bind(&visibility_label)
    .bind(&visibility_subject)
    .bind(&replay.provenance_kind)
    .bind(&replay.provenance_reference)
    .bind(&replay.provenance_recorded_by)
    .bind(replay.sequence)
    .execute(&mut **transaction)
    .await
    .map_err(database_error("replay_fork_scenario"))?;
    let matches: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM public.scenarios
             WHERE scenario_id = $1 AND campaign_id = $2
               AND content_hash = $3
               AND document_json = $4::JSONB
               AND visibility_label::TEXT = $5
               AND visibility_subject = $6
               AND last_event_sequence = $7
        )
        "#,
    )
    .bind(&scenario_id)
    .bind(child_campaign_id)
    .bind(&content_hash)
    .bind(&document_json)
    .bind(&visibility_label)
    .bind(&visibility_subject)
    .bind(replay.sequence)
    .fetch_one(&mut **transaction)
    .await
    .map_err(database_error("verify_replayed_fork_scenario"))?;
    if !matches {
        return Err(CoreDomainRepositoryError::Integrity(
            "fork_scenario_identity_conflict",
        ));
    }
    Ok(())
}
