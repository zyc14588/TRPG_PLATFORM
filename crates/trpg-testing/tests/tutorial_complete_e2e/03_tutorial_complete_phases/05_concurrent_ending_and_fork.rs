{
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM public.event_store \
             WHERE event_type = 'CharacterGrowthApplied'",
        )
        .fetch_one(&primary)
        .await
        .unwrap(),
        growth_events_before_duplicate,
        "semantic duplicate growth must be rejected before canonical append"
    );

    repository
        .request_reconsideration(
            &metadata(
                AUTHORITY_ID,
                PLAYER_ID,
                "investigator",
                "reconsideration_p08_tutorial",
                "reconsideration",
                0,
                "p08_reconsideration_request",
                "party_visible",
                "not_applicable",
                "user_statement",
            ),
            &RequestReconsiderationRequest {
                reconsideration_id: "reconsideration_p08_tutorial".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                original_event_sequence: campaign_event_sequence,
                requested_by: PLAYER_ID.to_owned(),
                reason: "Review the opening archive ruling".to_owned(),
            },
        )
        .await
        .expect("append a reconsideration request without rewriting history");
    repository
        .review_reconsideration(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "reconsideration_p08_tutorial",
                "reconsideration",
                1,
                "p08_reconsideration_review",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &ReviewReconsiderationRequest {
                reconsideration_id: "reconsideration_p08_tutorial".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                review_event_id: "review_event_p08_tutorial".to_owned(),
                review_summary: "The first ruling omitted the recovered signature".to_owned(),
            },
        )
        .await
        .expect("append the reconsideration review");
    repository
        .resolve_reconsideration(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "reconsideration_p08_tutorial",
                "reconsideration",
                2,
                "p08_reconsideration_resolve",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &ResolveReconsiderationRequest {
                reconsideration_id: "reconsideration_p08_tutorial".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                resolution_event_id: "resolution_event_p08_tutorial".to_owned(),
                outcome: ReconsiderationOutcome::Corrected,
                resolution: "Append a corrected ruling that admits the signature".to_owned(),
                corrected_event_type: Some("RulingCorrected".to_owned()),
                corrected_payload_json: Some(
                    r#"{"ruling":"signature admitted","supersedes_sequence":1}"#.to_owned(),
                ),
            },
        )
        .await
        .expect("append the correction while retaining the original event");

    repository
        .record_ending(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "ending_event_p08_later",
                "ending",
                0,
                "p08_later_ending",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &RecordEndingRequest {
                ending_event_id: "ending_event_p08_later".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                session_id: "session_p08_later".to_owned(),
                ending_id: "ending_expose_marta".to_owned(),
                summary: "A later session confirms the archive findings.".to_owned(),
                ended_at_unix_ms: NOW_MS + 12_000,
            },
        )
        .await
        .expect("record the later scenario-defined ending");
    let later_growth_roll =
        server_roll_skill_growth(growth_after).expect("server-owned later growth rolls");
    let later_growth_after = later_growth_roll.outcome().skill_after;
    repository
        .record_growth(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "growth_event_p08_later",
                "growth",
                0,
                "p08_later_growth",
                "private_to_player",
                PLAYER_ID,
                "rules_engine_decision",
            ),
            &RecordGrowthRequest {
                growth_event_id: "growth_event_p08_later".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                session_id: "session_p08_later".to_owned(),
                ending_event_id: "ending_event_p08_later".to_owned(),
                character_id: CHARACTER_ID.to_owned(),
                source_sheet_version_id: "sheet_p08_evelyn_v3".to_owned(),
                new_sheet_version_id: "sheet_p08_evelyn_v4".to_owned(),
                skill_name: "Library Use".to_owned(),
                growth_rolls: later_growth_roll.evidence().clone(),
            },
        )
        .await
        .expect("apply a later growth that is outside the source-session cutoff");

    repository
        .request_reconsideration(
            &metadata(
                AUTHORITY_ID,
                PLAYER_ID,
                "investigator",
                "reconsideration_p08_after_later",
                "reconsideration",
                0,
                "p08_reconsideration_after_later_request",
                "party_visible",
                "not_applicable",
                "user_statement",
            ),
            &RequestReconsiderationRequest {
                reconsideration_id: "reconsideration_p08_after_later".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                original_event_sequence: campaign_event_sequence,
                requested_by: PLAYER_ID.to_owned(),
                reason: "Confirm the old campaign ruling after later play".to_owned(),
            },
        )
        .await
        .expect("append a late reconsideration of source-session history");
    repository
        .review_reconsideration(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "reconsideration_p08_after_later",
                "reconsideration",
                1,
                "p08_reconsideration_after_later_review",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &ReviewReconsiderationRequest {
                reconsideration_id: "reconsideration_p08_after_later".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                review_event_id: "review_event_p08_after_later".to_owned(),
                review_summary: "Later play does not change the original ruling".to_owned(),
            },
        )
        .await
        .expect("review the late reconsideration");
    repository
        .resolve_reconsideration(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "reconsideration_p08_after_later",
                "reconsideration",
                2,
                "p08_reconsideration_after_later_resolve",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &ResolveReconsiderationRequest {
                reconsideration_id: "reconsideration_p08_after_later".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                resolution_event_id: "resolution_event_p08_after_later".to_owned(),
                outcome: ReconsiderationOutcome::Upheld,
                resolution: "The original campaign ruling remains valid".to_owned(),
                corrected_event_type: None,
                corrected_payload_json: None,
            },
        )
        .await
        .expect("resolve the late reconsideration without widening the source cutoff");

    repository
        .start_session(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "session_p08_concurrency",
                "session",
                0,
                "p08_concurrent_session_start",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &StartSessionRequest {
                session_id: "session_p08_concurrency".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                room_id: "room_p08_tutorial".to_owned(),
                scenario_id: "scenario_p08_tutorial".to_owned(),
                scene_id: "scene_p08_concurrency".to_owned(),
                scene_key: "scene_archive_front".to_owned(),
                scene_name: "并发结算验证".to_owned(),
                started_at_unix_ms: NOW_MS + 13_000,
            },
        )
        .await
        .expect("start a dedicated concurrent conclusion session");
    repository
        .change_session_state(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "session_p08_concurrency",
                "session",
                1,
                "p08_concurrent_session_end",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            CAMPAIGN_ID,
            "session_p08_concurrency",
            SessionState::Ended,
            NOW_MS + 14_000,
        )
        .await
        .expect("end the dedicated concurrent conclusion session");
    let ending_count_before_race: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM public.event_store WHERE event_type = 'EndingRecorded'",
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    let ending_race_metadata_a = metadata(
        AUTHORITY_ID,
        KEEPER_ID,
        "human_keeper",
        "ending_event_p08_race_a",
        "ending",
        0,
        "p08_ending_race_a",
        "party_visible",
        "not_applicable",
        "human_keeper_statement",
    );
    let ending_race_metadata_b = metadata(
        AUTHORITY_ID,
        KEEPER_ID,
        "human_keeper",
        "ending_event_p08_race_b",
        "ending",
        0,
        "p08_ending_race_b",
        "party_visible",
        "not_applicable",
        "human_keeper_statement",
    );
    let ending_race_request_a = RecordEndingRequest {
        ending_event_id: "ending_event_p08_race_a".to_owned(),
        campaign_id: CAMPAIGN_ID.to_owned(),
        session_id: "session_p08_concurrency".to_owned(),
        ending_id: "ending_expose_marta".to_owned(),
        summary: "Concurrent ending candidate A.".to_owned(),
        ended_at_unix_ms: NOW_MS + 15_000,
    };
    let ending_race_request_b = RecordEndingRequest {
        ending_event_id: "ending_event_p08_race_b".to_owned(),
        campaign_id: CAMPAIGN_ID.to_owned(),
        session_id: "session_p08_concurrency".to_owned(),
        ending_id: "ending_expose_marta".to_owned(),
        summary: "Concurrent ending candidate B.".to_owned(),
        ended_at_unix_ms: NOW_MS + 15_001,
    };
    let (ending_race_a, ending_race_b) = tokio::join!(
        repository.record_ending(&ending_race_metadata_a, &ending_race_request_a),
        repository.record_ending(&ending_race_metadata_b, &ending_race_request_b),
    );
    assert_eq!(
        usize::from(ending_race_a.is_ok()) + usize::from(ending_race_b.is_ok()),
        1,
        "the session advisory lock must allow exactly one ending"
    );
    let ending_race_failure = if ending_race_a.is_err() {
        &ending_race_a
    } else {
        &ending_race_b
    };
    assert!(matches!(
        ending_race_failure,
        Err(CoreDomainRepositoryError::Integrity(
            "ending_session_already_recorded"
        ))
    ));
    include!("06_child_materialization_validation.rs");
}
