{
    assert!(restarted_repository
        .switch_scene(
            &metadata(
                &winning_session_id,
                "session",
                "scene.switch",
                3,
                "illegal_paused_scene_switch",
            ),
            &SwitchSceneRequest {
                session_id: winning_session_id.clone(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                next_scene_id: "scene_p06_runtime_illegal".to_owned(),
                next_scene_key: "illegal_while_paused".to_owned(),
                next_scene_name: "Illegal scene".to_owned(),
                switched_at_unix_ms: NOW_MS + 23,
            },
        )
        .await
        .is_err());
    let resume_metadata = metadata(
        &winning_session_id,
        "session",
        "session.resume",
        3,
        "runtime_resume",
    );
    let resumed = restarted_repository
        .change_session_state(
            &resume_metadata,
            CAMPAIGN_ID,
            &winning_session_id,
            SessionState::Active,
            NOW_MS + 24,
        )
        .await
        .expect("resume paused session");
    let resumed_retry = restarted_repository
        .change_session_state(
            &resume_metadata,
            CAMPAIGN_ID,
            &winning_session_id,
            SessionState::Active,
            NOW_MS + 24,
        )
        .await
        .expect("exact resume retry is idempotent");
    assert_eq!(
        resumed_retry.last_event_sequence,
        resumed.last_event_sequence
    );
    machine.resume().unwrap();
    let end_metadata = metadata(
        &winning_session_id,
        "session",
        "session.end",
        4,
        "runtime_end",
    );
    let ended = restarted_repository
        .change_session_state(
            &end_metadata,
            CAMPAIGN_ID,
            &winning_session_id,
            SessionState::Ended,
            NOW_MS + 25,
        )
        .await
        .expect("end resumed session");
    let ended_retry = restarted_repository
        .change_session_state(
            &end_metadata,
            CAMPAIGN_ID,
            &winning_session_id,
            SessionState::Ended,
            NOW_MS + 25,
        )
        .await
        .expect("exact end retry is idempotent");
    assert_eq!(ended_retry.last_event_sequence, ended.last_event_sequence);
    machine.end().unwrap();
    assert_eq!(machine.state(), DurableSessionState::Ended);
    assert_eq!(machine.version(), 5);
    assert_eq!(
        machine.active_scene().unwrap().state,
        DurableSceneState::Closed
    );

    restarted_repository
        .start_session(
            &metadata(
                "session_p06_runtime_after_end",
                "session",
                "session.start",
                0,
                "start_after_end",
            ),
            &session_request(
                "session_p06_runtime_after_end",
                "scene_p06_runtime_after_end",
                30,
            ),
        )
        .await
        .expect("ended session releases the room for a new start");
}
