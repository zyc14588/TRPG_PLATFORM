
fn parse_session_state(value: &str) -> Result<SessionState, CoreDomainRepositoryError> {
    match value {
        "SCHEDULED" => Ok(SessionState::Scheduled),
        "ACTIVE" => Ok(SessionState::Active),
        "PAUSED" => Ok(SessionState::Paused),
        "ENDED" => Ok(SessionState::Ended),
        _ => Err(CoreDomainRepositoryError::Integrity(
            "unknown_session_state",
        )),
    }
}

fn session_state_action(state: SessionState) -> &'static str {
    match state {
        SessionState::Scheduled => "session.schedule",
        SessionState::Active => "session.resume",
        SessionState::Paused => "session.pause",
        SessionState::Ended => "session.end",
    }
}

#[allow(clippy::too_many_arguments)]
async fn project_gameplay_roll_consumptions(
    transaction: &mut Transaction<'_, Postgres>,
    consumptions: &[GameplayRollConsumption],
    campaign_id: &str,
    aggregate_kind: &str,
    aggregate_id: &str,
    visibility_label: &str,
    visibility_subject: &str,
    provenance_kind: &str,
    provenance_reference: &str,
    provenance_recorded_by: &str,
    last_event_sequence: i64,
) -> Result<(), CoreDomainRepositoryError> {
    for consumption in consumptions {
        let inserted = sqlx::query(
            r#"
            INSERT INTO public.gameplay_roll_consumptions (
                roll_id, campaign_id, aggregate_kind, aggregate_id, roll_kind,
                random_source, visibility_label, visibility_subject,
                provenance_kind, provenance_reference, provenance_recorded_by,
                last_event_sequence
            ) VALUES (
                $1, $2, $3, $4, $5, 'SERVER_OS_CSPRNG', $6, $7,
                $8, $9, $10, $11
            )
            ON CONFLICT (roll_id) DO NOTHING
            "#,
        )
        .bind(&consumption.roll_id)
        .bind(campaign_id)
        .bind(aggregate_kind)
        .bind(aggregate_id)
        .bind(consumption.roll_kind)
        .bind(visibility_label)
        .bind(visibility_subject)
        .bind(provenance_kind)
        .bind(provenance_reference)
        .bind(provenance_recorded_by)
        .bind(last_event_sequence)
        .execute(&mut **transaction)
        .await
        .map_err(database_error("project_gameplay_roll_consumption"))?;
        if inserted.rows_affected() != 1 {
            let existing = sqlx::query(
                r#"
                SELECT campaign_id, aggregate_kind, aggregate_id, roll_kind,
                       random_source, visibility_label::TEXT AS visibility_label,
                       visibility_subject,
                       provenance_kind::TEXT AS provenance_kind,
                       provenance_reference, provenance_recorded_by,
                       last_event_sequence
                  FROM public.gameplay_roll_consumptions
                 WHERE roll_id = $1
                "#,
            )
            .bind(&consumption.roll_id)
            .fetch_optional(&mut **transaction)
            .await
            .map_err(database_error("load_projected_gameplay_roll_consumption"))?
            .ok_or(CoreDomainRepositoryError::Integrity(
                "gameplay_roll_reservation_missing",
            ))?;
            if existing.get::<String, _>("campaign_id") != campaign_id
                || existing.get::<String, _>("aggregate_kind") != aggregate_kind
                || existing.get::<String, _>("aggregate_id") != aggregate_id
                || existing.get::<String, _>("roll_kind") != consumption.roll_kind
                || existing.get::<String, _>("random_source") != "SERVER_OS_CSPRNG"
                || existing.get::<String, _>("visibility_label") != visibility_label
                || existing.get::<String, _>("visibility_subject") != visibility_subject
                || existing.get::<String, _>("provenance_kind") != provenance_kind
                || existing.get::<String, _>("provenance_reference") != provenance_reference
                || existing.get::<String, _>("provenance_recorded_by") != provenance_recorded_by
                || existing.get::<i64, _>("last_event_sequence") != last_event_sequence
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "gameplay_roll_already_consumed",
                ));
            }
        }
    }
    Ok(())
}

async fn ensure_scenario_scene_key(
    transaction: &mut Transaction<'_, Postgres>,
    campaign_id: &str,
    scenario_id: &str,
    scene_key: &str,
) -> Result<(), CoreDomainRepositoryError> {
    let exists: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1
              FROM public.scenarios AS scenario
              CROSS JOIN LATERAL jsonb_array_elements(
                  CASE
                      WHEN jsonb_typeof(scenario.document_json -> 'scenes') =
                           'array'
                      THEN scenario.document_json -> 'scenes'
                      ELSE '[]'::JSONB
                  END
              ) AS scene
             WHERE scenario.campaign_id = $1
               AND scenario.scenario_id = $2
               AND scenario.validated
               AND scene ->> 'id' = $3
        )
        "#,
    )
    .bind(campaign_id)
    .bind(scenario_id)
    .bind(scene_key)
    .fetch_one(&mut **transaction)
    .await
    .map_err(database_error("validate_scenario_scene_key"))?;
    if !exists {
        return Err(CoreDomainRepositoryError::InvalidInput(
            "scenario_scene_key",
        ));
    }
    Ok(())
}
