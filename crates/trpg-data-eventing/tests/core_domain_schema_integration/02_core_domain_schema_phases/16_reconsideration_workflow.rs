{
    repository
        .review_reconsideration(
            &metadata(
                CAMPAIGN_ID,
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "reconsideration_p06_schema",
                "reconsideration",
                "reconsideration.review",
                1,
                "reconsideration_review",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &ReviewReconsiderationRequest {
                reconsideration_id: "reconsideration_p06_schema".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                review_event_id: "review_event_p06_schema".to_owned(),
                review_summary: "  The original ruling omitted a material clue  ".to_owned(),
            },
        )
        .await
        .expect("append review event without rewriting the original event");
    repository
        .resolve_reconsideration(
            &metadata(
                CAMPAIGN_ID,
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "reconsideration_p06_schema",
                "reconsideration",
                "reconsideration.resolve",
                2,
                "reconsideration_resolution",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &ResolveReconsiderationRequest {
                reconsideration_id: "reconsideration_p06_schema".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                resolution_event_id: "resolution_event_p06_schema".to_owned(),
                outcome: ReconsiderationOutcome::Corrected,
                resolution: "  Append a corrected ruling that includes the clue  ".to_owned(),
                corrected_event_type: Some("RulingCorrected".to_owned()),
                corrected_payload_json: Some(
                    r#"{"ruling":"clue admitted","supersedes_sequence":1}"#.to_owned(),
                ),
            },
        )
        .await
        .expect("append a correction event and resolve reconsideration");
    let reconsideration = sqlx::query(
        r#"
        SELECT state, outcome, review_summary, resolution, version,
               jsonb_array_length(event_chain) AS chain_length
          FROM public.reconsiderations
         WHERE reconsideration_id = 'reconsideration_p06_schema'
        "#,
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(reconsideration.get::<String, _>("state"), "RESOLVED");
    assert_eq!(reconsideration.get::<String, _>("outcome"), "CORRECTED");
    assert_eq!(
        reconsideration.get::<String, _>("review_summary"),
        "The original ruling omitted a material clue"
    );
    assert_eq!(
        reconsideration.get::<String, _>("resolution"),
        "Append a corrected ruling that includes the clue"
    );
    assert_eq!(reconsideration.get::<i64, _>("version"), 3);
    assert_eq!(reconsideration.get::<i32, _>("chain_length"), 3);
    let reconsideration_events = canonical_reader
        .load_replay_page(CAMPAIGN_ID, 0, 500)
        .await
        .unwrap();
    let reconsideration_event_text = (
        reconsideration_events
            .iter()
            .find(|event| {
                event.event_type == "ReconsiderationReviewed"
                    && event.stream_id == "reconsideration_p06_schema"
            })
            .and_then(|event| {
                event
                    .payload
                    .pointer("/data/review_summary")
                    .and_then(serde_json::Value::as_str)
            })
            .expect("load canonical reconsideration review")
            .to_owned(),
        reconsideration_events
            .iter()
            .find(|event| {
                event.event_type == "ReconsiderationCorrected"
                    && event.stream_id == "reconsideration_p06_schema"
            })
            .and_then(|event| {
                event
                    .payload
                    .pointer("/data/resolution")
                    .and_then(serde_json::Value::as_str)
            })
            .expect("load canonical reconsideration resolution")
            .to_owned(),
    );
    assert_eq!(
        reconsideration_event_text,
        (
            "The original ruling omitted a material clue".to_owned(),
            "Append a corrected ruling that includes the clue".to_owned(),
        ),
        "reconsideration events must be normalized before their projections are written"
    );
    let original_event_still_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM public.event_store WHERE sequence = $1)")
            .bind(campaign_event_sequence)
            .fetch_one(&primary)
            .await
            .unwrap();
    assert!(
        original_event_still_exists,
        "a corrected reconsideration must never delete its original event"
    );

    repository
        .request_reconsideration(
            &metadata(
                CAMPAIGN_ID,
                AUTHORITY_ID,
                PLAYER_ID,
                "investigator",
                "reconsideration_p08_upheld",
                "reconsideration",
                "reconsideration.request",
                0,
                "reconsideration_upheld_request",
                "party_visible",
                "not_applicable",
                "user_statement",
            ),
            &RequestReconsiderationRequest {
                reconsideration_id: "reconsideration_p08_upheld".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                original_event_sequence: campaign_event_sequence,
                requested_by: PLAYER_ID.to_owned(),
                reason: "Request a second review of the opening ruling".to_owned(),
            },
        )
        .await
        .expect("append reconsideration request for upheld path");
    repository
        .review_reconsideration(
            &metadata(
                CAMPAIGN_ID,
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "reconsideration_p08_upheld",
                "reconsideration",
                "reconsideration.review",
                1,
                "reconsideration_upheld_review",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &ReviewReconsiderationRequest {
                reconsideration_id: "reconsideration_p08_upheld".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                review_event_id: "review_event_p08_upheld".to_owned(),
                review_summary: "The original evidence and rule citation are complete".to_owned(),
            },
        )
        .await
        .expect("append review for upheld path");
    repository
        .resolve_reconsideration(
            &metadata(
                CAMPAIGN_ID,
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "reconsideration_p08_upheld",
                "reconsideration",
                "reconsideration.resolve",
                2,
                "reconsideration_upheld_resolution",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &ResolveReconsiderationRequest {
                reconsideration_id: "reconsideration_p08_upheld".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                resolution_event_id: "resolution_event_p08_upheld".to_owned(),
                outcome: ReconsiderationOutcome::Upheld,
                resolution: "Original ruling upheld after review".to_owned(),
                corrected_event_type: None,
                corrected_payload_json: None,
            },
        )
        .await
        .expect("append upheld resolution without a correction payload");
    let upheld_outcome: String = sqlx::query_scalar(
        "SELECT outcome FROM public.reconsiderations \
         WHERE reconsideration_id = 'reconsideration_p08_upheld'",
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(upheld_outcome, "UPHELD");

    let p08_projection_before: serde_json::Value = sqlx::query_scalar(
        r#"
        SELECT jsonb_build_object(
            'combat', (SELECT to_jsonb(combat) FROM public.combat_states AS combat
                        WHERE combat.campaign_id = $1),
            'chase', (SELECT to_jsonb(chase) FROM public.chase_states AS chase
                       WHERE chase.campaign_id = $1),
            'roll_consumptions', (
                SELECT jsonb_agg(to_jsonb(consumption)
                                 ORDER BY consumption.roll_id)
                  FROM public.gameplay_roll_consumptions AS consumption
                 WHERE consumption.campaign_id = $1
            ),
            'ending', (SELECT to_jsonb(ending) FROM public.ending_events AS ending
                        WHERE ending.campaign_id = $1),
            'growth', (SELECT to_jsonb(growth) FROM public.growth_events AS growth
                        WHERE growth.campaign_id = $1),
            'growth_sheet', (
                SELECT to_jsonb(sheet)
                  FROM public.character_sheet_versions AS sheet
                 WHERE sheet.sheet_version_id = 'sheet_p06_player_v2'
            ),
            'character', (
                SELECT to_jsonb(character)
                  FROM public.characters AS character
                 WHERE character.character_id = 'character_p06_player'
            ),
            'reconsiderations', (
                SELECT jsonb_agg(to_jsonb(reconsideration)
                                 ORDER BY reconsideration.reconsideration_id)
                  FROM public.reconsiderations AS reconsideration
                 WHERE reconsideration.campaign_id = $1
            )
        )
        "#,
    )
    .bind(CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .unwrap();
    let event_count_before: i64 =
        sqlx::query_scalar("SELECT count(*) FROM public.event_store WHERE campaign_id = $1")
            .bind(CAMPAIGN_ID)
            .fetch_one(&primary)
            .await
            .unwrap();
    let mut corrupt_p08_projection = primary.begin().await.unwrap();
    for statement in [
        "ALTER TABLE public.combat_states DISABLE TRIGGER combat_states_event_guard",
        "ALTER TABLE public.chase_states DISABLE TRIGGER chase_states_event_guard",
        "ALTER TABLE public.ending_events DISABLE TRIGGER ending_events_event_guard",
        "ALTER TABLE public.growth_events DISABLE TRIGGER growth_events_event_guard",
        "ALTER TABLE public.reconsiderations DISABLE TRIGGER reconsiderations_event_guard",
        "ALTER TABLE public.characters DISABLE TRIGGER characters_event_guard",
        "ALTER TABLE public.character_sheet_versions DISABLE TRIGGER character_sheet_versions_event_guard",
    ] {
        sqlx::query(statement)
            .execute(&mut *corrupt_p08_projection)
            .await
            .unwrap();
    }
    sqlx::query(
        r#"
        UPDATE public.combat_states
           SET state_json = jsonb_set(state_json, '{corrupted}', 'true'::jsonb),
               provenance_reference = 'corrupted_same_version'
         WHERE campaign_id = $1
        "#,
    )
    .bind(CAMPAIGN_ID)
    .execute(&mut *corrupt_p08_projection)
    .await
    .unwrap();
    for statement in [
        "UPDATE public.ending_events \
         SET summary = 'CORRUPTED ENDING', \
             provenance_reference = 'corrupted_same_version' \
         WHERE campaign_id = $1",
        "UPDATE public.growth_events \
         SET provenance_reference = 'corrupted_same_version' \
         WHERE campaign_id = $1",
        "UPDATE public.reconsiderations \
         SET review_summary = 'CORRUPTED REVIEW', \
             provenance_reference = 'corrupted_same_version' \
         WHERE campaign_id = $1",
        "UPDATE public.characters \
         SET provenance_reference = 'corrupted_same_version' \
         WHERE campaign_id = $1 AND character_id = 'character_p06_player'",
        "UPDATE public.character_sheet_versions \
         SET sheet_json = jsonb_set(sheet_json, '{corrupted}', 'true'::jsonb) \
         WHERE campaign_id = $1 AND sheet_version_id = 'sheet_p06_player_v2'",
    ] {
        sqlx::query(statement)
            .bind(CAMPAIGN_ID)
            .execute(&mut *corrupt_p08_projection)
            .await
            .unwrap();
    }
    sqlx::query(
        r#"
        UPDATE public.chase_states
           SET state_json = jsonb_set(state_json, '{corrupted}', 'true'::jsonb),
               provenance_reference = 'corrupted_same_version'
         WHERE campaign_id = $1
        "#,
    )
    .bind(CAMPAIGN_ID)
    .execute(&mut *corrupt_p08_projection)
    .await
    .unwrap();
    include!("17_projection_corruption_injection.rs");
}
