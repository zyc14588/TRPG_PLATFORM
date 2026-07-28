{

    Box::pin(async {
        let ending_metadata = metadata(
            CAMPAIGN_ID,
            AUTHORITY_ID,
            KEEPER_ID,
            "human_keeper",
            "ending_event_p08_schema",
            "ending",
            "ending.record",
            0,
            "ending_p08_record",
            "party_visible",
            "not_applicable",
            "human_keeper_statement",
        );
        let ending_request = RecordEndingRequest {
            ending_event_id: "ending_event_p08_schema".to_owned(),
            campaign_id: CAMPAIGN_ID.to_owned(),
            session_id: "session_p06_schema".to_owned(),
            ending_id: "  ending_expose_marta  ".to_owned(),
            summary: "  The investigators expose Marta and preserve the archive.  ".to_owned(),
            ended_at_unix_ms: NOW_MS + 7_000,
        };
        sqlx::raw_sql(
            r#"
        CREATE FUNCTION public.reject_p08_ending_projection_for_test()
        RETURNS trigger
        LANGUAGE plpgsql
        AS $$
        BEGIN
            IF NEW.ending_event_id = 'ending_event_p08_schema' THEN
                RAISE EXCEPTION 'injected P08 ending projection failure';
            END IF;
            RETURN NEW;
        END;
        $$;
        CREATE TRIGGER zz_reject_p08_ending_projection_for_test
        BEFORE INSERT ON public.ending_events
        FOR EACH ROW EXECUTE FUNCTION
            public.reject_p08_ending_projection_for_test();
        "#,
        )
        .execute(&primary)
        .await
        .expect("install P08 ending projection failure injection");
        assert!(matches!(
            repository
                .record_ending(&ending_metadata, &ending_request)
                .await,
            Err(CoreDomainRepositoryError::Database("project_ending"))
        ));
        let reserved_ending_sequence: i64 = sqlx::query_scalar(
            "SELECT event_sequence \
           FROM core_domain.session_ending_reservations \
          WHERE session_id = 'session_p06_schema'",
        )
        .fetch_one(&primary)
        .await
        .expect("the canonical transaction reserves the Session ending");
        let reservation_session_fk: (bool, bool) = sqlx::query_as(
            r#"
            SELECT condeferrable, condeferred
              FROM pg_constraint
             WHERE conrelid =
                   'core_domain.session_ending_reservations'::regclass
               AND conname =
                   'session_ending_reservations_session_id_fkey'
            "#,
        )
        .fetch_one(&primary)
        .await
        .expect("load the Session ending reservation foreign key");
        assert_eq!(
            reservation_session_fk,
            (true, true),
            "projection rebuild must be able to recreate a referenced fork Session"
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM public.ending_events \
             WHERE session_id = 'session_p06_schema'",
            )
            .fetch_one(&primary)
            .await
            .unwrap(),
            0,
            "the injected projection failure must not erase the canonical reservation"
        );
        assert!(matches!(
            repository
                .record_ending(
                    &metadata(
                        CAMPAIGN_ID,
                        AUTHORITY_ID,
                        KEEPER_ID,
                        "human_keeper",
                        "ending_event_p08_after_projection_failure",
                        "ending",
                        "ending.record",
                        0,
                        "ending_p08_after_projection_failure",
                        "party_visible",
                        "not_applicable",
                        "human_keeper_statement",
                    ),
                    &RecordEndingRequest {
                        ending_event_id: "ending_event_p08_after_projection_failure".to_owned(),
                        campaign_id: CAMPAIGN_ID.to_owned(),
                        session_id: "session_p06_schema".to_owned(),
                        ending_id: "ending_expose_marta".to_owned(),
                        summary: "A projector crash cannot authorize a second ending.".to_owned(),
                        ended_at_unix_ms: NOW_MS + 7_001,
                    },
                )
                .await,
            Err(CoreDomainRepositoryError::Integrity(
                "ending_session_already_recorded"
            ))
        ));
        sqlx::raw_sql(
            r#"
        DROP TRIGGER zz_reject_p08_ending_projection_for_test
            ON public.ending_events;
        DROP FUNCTION public.reject_p08_ending_projection_for_test();
        "#,
        )
        .execute(&primary)
        .await
        .expect("remove P08 ending projection failure injection");
        repository
            .record_ending(&ending_metadata, &ending_request)
            .await
            .expect("exact retry projects the canonically reserved ending");
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT last_event_sequence FROM public.ending_events \
             WHERE ending_event_id = 'ending_event_p08_schema'",
            )
            .fetch_one(&primary)
            .await
            .unwrap(),
            reserved_ending_sequence,
            "projection recovery must reuse the original canonical ending"
        );
    })
    .await;
    let normalized_ending_projection: (String, String) = sqlx::query_as(
        "SELECT ending_id, summary FROM public.ending_events \
         WHERE ending_event_id = 'ending_event_p08_schema'",
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    let normalized_ending_event = canonical_reader
        .load_replay_page(CAMPAIGN_ID, 0, 500)
        .await
        .unwrap()
        .into_iter()
        .find(|event| event.event_type == "EndingRecorded")
        .and_then(|event| {
            Some((
                event
                    .payload
                    .pointer("/data/ending_id")?
                    .as_str()?
                    .to_owned(),
                event.payload.pointer("/data/summary")?.as_str()?.to_owned(),
            ))
        })
        .expect("load the normalized canonical ending");
    assert_eq!(
        normalized_ending_projection,
        (
            "ending_expose_marta".to_owned(),
            "The investigators expose Marta and preserve the archive.".to_owned(),
        )
    );
    assert_eq!(
        normalized_ending_event, normalized_ending_projection,
        "the canonical event and live ending projection must share one normalized ending"
    );
    let conflicting_ending_identity_metadata = metadata(
        CAMPAIGN_ID,
        AUTHORITY_ID,
        KEEPER_ID,
        "human_keeper",
        "ending_event_p08_schema",
        "ending",
        "ending.record",
        0,
        "ending_p08_conflicting_identity",
        "party_visible",
        "not_applicable",
        "human_keeper_statement",
    );
    assert!(matches!(
        repository
            .record_ending(
                &conflicting_ending_identity_metadata,
                &RecordEndingRequest {
                    ending_event_id: "ending_event_p08_schema".to_owned(),
                    campaign_id: CAMPAIGN_ID.to_owned(),
                    session_id: "session_p06_schema".to_owned(),
                    ending_id: "ending_expose_marta".to_owned(),
                    summary: "A new command cannot reuse the ending identity.".to_owned(),
                    ended_at_unix_ms: NOW_MS + 7_000,
                },
            )
            .await,
        Err(CoreDomainRepositoryError::Integrity(
            "ending_identity_conflict"
        ))
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM public.event_store \
             WHERE campaign_id = $1 AND event_type = 'EndingRecorded'",
        )
        .bind(CAMPAIGN_ID)
        .fetch_one(&primary)
        .await
        .unwrap(),
        ending_events_before_invalid_timestamp + 1,
        "a conflicting ending identity must fail before canonical append"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM public.formal_commits WHERE commit_id = $1",
        )
        .bind(&conflicting_ending_identity_metadata.commit_id)
        .fetch_one(&primary)
        .await
        .unwrap(),
        0,
        "a conflicting ending identity must not leave a committed formal write"
    );
    let combat_health_source: (String, i64, String, String) = sqlx::query_as(
        r#"
        SELECT sheet.sheet_version_id,
               character.current_sheet_version,
               sheet.sheet_json #>> '{combat_profile,current_hp}',
               sheet.sheet_json #>> '{combat_profile,condition}'
          FROM public.characters AS character
          JOIN public.character_sheet_versions AS sheet
            ON sheet.character_id = character.character_id
           AND sheet.version = character.current_sheet_version
         WHERE character.character_id = 'character_p06_player'
           AND character.visibility_label::TEXT = 'private_to_player'
           AND sheet.visibility_label::TEXT = 'private_to_player'
        "#,
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(
        (
            combat_health_source.1,
            combat_health_source.2.as_str(),
            combat_health_source.3.as_str(),
        ),
        (3, "5", "ABLE"),
        "successful First Aid must create another private sheet version"
    );
    let growth_roll = server_roll_skill_growth(70).unwrap();
    let growth_outcome = *growth_roll.outcome();
    let growth_events_before_reuse: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM public.event_store \
         WHERE campaign_id = $1 AND event_type = 'CharacterGrowthApplied'",
    )
    .bind(CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .unwrap();
    let conflicting_growth_sheet_metadata = metadata(
        CAMPAIGN_ID,
        AUTHORITY_ID,
        KEEPER_ID,
        "human_keeper",
        "growth_event_p08_conflicting_sheet",
        "growth",
        "growth.record",
        0,
        "growth_p08_conflicting_sheet",
        "private_to_player",
        PLAYER_ID,
        "rules_engine_decision",
    );
    assert!(matches!(
        repository
            .record_growth(
                &conflicting_growth_sheet_metadata,
                &RecordGrowthRequest {
                    growth_event_id: "growth_event_p08_conflicting_sheet".to_owned(),
                    campaign_id: CAMPAIGN_ID.to_owned(),
                    session_id: "session_p06_schema".to_owned(),
                    ending_event_id: "ending_event_p08_schema".to_owned(),
                    character_id: "character_p06_player".to_owned(),
                    source_sheet_version_id: combat_health_source.0.clone(),
                    new_sheet_version_id: "sheet_p08_keeper_private_v1".to_owned(),
                    skill_name: "Library Use".to_owned(),
                    growth_rolls: growth_roll.evidence().clone(),
                },
            )
            .await,
        Err(CoreDomainRepositoryError::Integrity(
            "growth_sheet_identity_conflict"
        ))
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM public.event_store \
             WHERE campaign_id = $1 AND event_type = 'CharacterGrowthApplied'",
        )
        .bind(CAMPAIGN_ID)
        .fetch_one(&primary)
        .await
        .unwrap(),
        growth_events_before_reuse,
        "a conflicting Growth sheet ID must fail before canonical append"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM public.formal_commits WHERE commit_id = $1",
        )
        .bind(&conflicting_growth_sheet_metadata.commit_id)
        .fetch_one(&primary)
        .await
        .unwrap(),
        0,
        "a conflicting Growth sheet ID must not leave a committed formal write"
    );
    include!("10_growth_roll_reuse_guards.rs");
}
