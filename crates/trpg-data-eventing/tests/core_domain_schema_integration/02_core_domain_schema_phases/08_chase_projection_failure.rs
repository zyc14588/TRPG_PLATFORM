{
    sqlx::raw_sql(
        r#"
        CREATE FUNCTION public.reject_terminal_chase_projection_for_test()
        RETURNS trigger
        LANGUAGE plpgsql
        AS $$
        BEGIN
            IF NEW.chase_id = 'chase_p08_schema'
               AND NEW.version = 2 THEN
                RAISE EXCEPTION
                    'injected terminal Chase projection failure';
            END IF;
            RETURN NEW;
        END;
        $$;
        CREATE TRIGGER zz_reject_terminal_chase_projection_for_test
        BEFORE INSERT OR UPDATE ON public.chase_states
        FOR EACH ROW EXECUTE FUNCTION
            public.reject_terminal_chase_projection_for_test();
        "#,
    )
    .execute(&primary)
    .await
    .expect("install terminal Chase projection failure injection");
    assert!(matches!(
        repository
            .record_chase_state(&terminal_chase_metadata, &terminal_chase_request,)
            .await,
        Err(CoreDomainRepositoryError::Database("project_chase_state"))
    ));
    assert!(chase
        .advance(
            &[
                percentile_with_result(40, true),
                percentile_with_result(40, false),
            ],
            None,
        )
        .is_err());
    let p08_roll_consumptions_before: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM public.gameplay_roll_consumptions \
         WHERE campaign_id = $1",
    )
    .bind(CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .unwrap();
    assert!(
        p08_roll_consumptions_before >= 3,
        "combat and chase transitions must project consumed server-roll evidence"
    );

    repository
        .change_session_state(
            &metadata(
                CAMPAIGN_ID,
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "session_p06_schema",
                "session",
                "session.end",
                4,
                "session_end",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            CAMPAIGN_ID,
            "session_p06_schema",
            SessionState::Ended,
            NOW_MS + 6_000,
        )
        .await
        .expect("end resumed session");
    let terminal_event_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM public.event_store \
         WHERE campaign_id = $1 \
           AND stream_id IN ('combat_p08_schema', 'chase_p08_schema')",
    )
    .bind(CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .unwrap();
    sqlx::raw_sql(
        r#"
        DROP TRIGGER zz_reject_terminal_combat_projection_for_test
            ON public.combat_states;
        DROP FUNCTION public.reject_terminal_combat_projection_for_test();
        DROP TRIGGER zz_reject_terminal_chase_projection_for_test
            ON public.chase_states;
        DROP FUNCTION public.reject_terminal_chase_projection_for_test();
        "#,
    )
    .execute(&primary)
    .await
    .expect("remove terminal gameplay projection failure injections");
    repository
        .record_combat_state(&terminal_combat_metadata, &terminal_combat_request)
        .await
        .expect("recover the terminal Combat projection after Session end");
    repository
        .record_chase_state(&terminal_chase_metadata, &terminal_chase_request)
        .await
        .expect("recover the terminal Chase projection after Session end");
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM public.event_store \
             WHERE campaign_id = $1 \
               AND stream_id IN ('combat_p08_schema', 'chase_p08_schema')",
        )
        .bind(CAMPAIGN_ID)
        .fetch_one(&primary)
        .await
        .unwrap(),
        terminal_event_count,
        "terminal projection recovery must reuse the exact canonical events"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) \
               FROM public.combat_states AS combat \
               JOIN public.chase_states AS chase \
                 ON chase.campaign_id = combat.campaign_id \
              WHERE combat.combat_id = 'combat_p08_schema' \
                AND combat.status = 'ENDED' \
                AND chase.chase_id = 'chase_p08_schema' \
                AND chase.status = 'CAUGHT'",
        )
        .fetch_one(&primary)
        .await
        .unwrap(),
        1,
        "exact retries must restore both terminal projections after Session end"
    );

    let ending_events_before_invalid_timestamp: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM public.event_store \
         WHERE campaign_id = $1 AND event_type = 'EndingRecorded'",
    )
    .bind(CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .unwrap();
    let invalid_ending_timestamp_metadata = metadata(
        CAMPAIGN_ID,
        AUTHORITY_ID,
        KEEPER_ID,
        "human_keeper",
        "ending_event_p08_invalid_timestamp",
        "ending",
        "ending.record",
        0,
        "ending_p08_invalid_timestamp",
        "party_visible",
        "not_applicable",
        "human_keeper_statement",
    );
    assert!(matches!(
        repository
            .record_ending(
                &invalid_ending_timestamp_metadata,
                &RecordEndingRequest {
                    ending_event_id: "ending_event_p08_invalid_timestamp".to_owned(),
                    campaign_id: CAMPAIGN_ID.to_owned(),
                    session_id: "session_p06_schema".to_owned(),
                    ending_id: "ending_expose_marta".to_owned(),
                    summary: "This timestamp cannot be represented.".to_owned(),
                    ended_at_unix_ms: u64::MAX,
                },
            )
            .await,
        Err(CoreDomainRepositoryError::InvalidInput("ending_timestamp"))
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
        ending_events_before_invalid_timestamp,
        "an unrepresentable ending timestamp must fail before canonical append"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM public.formal_commits WHERE commit_id = $1",
        )
        .bind(&invalid_ending_timestamp_metadata.commit_id)
        .fetch_one(&primary)
        .await
        .unwrap(),
        0,
        "invalid ending input must not leave a committed formal write"
    );
    include!("09_ending_and_growth_setup.rs");
}
