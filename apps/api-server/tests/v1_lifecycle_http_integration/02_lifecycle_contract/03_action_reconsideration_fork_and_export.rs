{

    let action_id = "action_ar06_http";
    let submitted_action = call(
        &application,
        "POST",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/player-actions"),
        Some(&player_token),
        Some(json!({
            "command": command("action_submit", 0),
            "campaign_id": CAMPAIGN_ID,
            "action_id": action_id,
            "character_id": CHARACTER_ID,
            "scene_id": "scene_ar06_basement",
            "submitted_at_unix_ms": now + 4_000,
            "intent": {
                "kind": "INVESTIGATION",
                "skill_name": "Library Use",
                "clue_id": "clue_wrong_signature",
                "clue_importance": "CORE",
                "adjustment": "NONE"
            }
        })),
    );
    expect_status(&submitted_action, 202, "Player action submit");
    assert_eq!(
        submitted_action.body["state"],
        "AWAITING_HUMAN_CONFIRMATION"
    );
    let unauthorized_confirmation = call(
        &application,
        "POST",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/player-actions/{action_id}/confirm"),
        Some(&outsider_token),
        Some(json!({
            "command": command("action_unauthorized_confirm", 1),
            "campaign_id": CAMPAIGN_ID,
            "action_id": action_id,
            "resolved_at_unix_ms": now + 5_000
        })),
    );
    expect_status(
        &unauthorized_confirmation,
        403,
        "unauthorized action confirmation",
    );
    let confirmed_action = call(
        &application,
        "POST",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/player-actions/{action_id}/confirm"),
        Some(&keeper_token),
        Some(json!({
            "command": command("action_confirm", 1),
            "campaign_id": CAMPAIGN_ID,
            "action_id": action_id,
            "resolved_at_unix_ms": now + 5_000
        })),
    );
    expect_status(&confirmed_action, 200, "Player action confirmation");
    assert_eq!(confirmed_action.body["state"], "RESOLVED");
    let action_sequence = confirmed_action.body["last_event_sequence"]
        .as_i64()
        .expect("confirmed action sequence");

    let reconsideration_id = "reconsideration_ar06_http";
    let reconsideration = call(
        &application,
        "POST",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/reconsiderations"),
        Some(&player_token),
        Some(json!({
            "command": command("reconsider_request", 0),
            "reconsideration_id": reconsideration_id,
            "campaign_id": CAMPAIGN_ID,
            "original_event_sequence": action_sequence,
            "requested_by": PLAYER_ID,
            "reason": "Review the canonical decision"
        })),
    );
    expect_status(&reconsideration, 202, "Reconsideration request");
    let reconsideration_review = call(
        &application,
        "POST",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/reconsiderations/{reconsideration_id}/review"),
        Some(&keeper_token),
        Some(json!({
            "command": command("reconsider_review", 1),
            "reconsideration_id": reconsideration_id,
            "campaign_id": CAMPAIGN_ID,
            "review_event_id": "review_event_ar06_http",
            "review_summary": "Canonical evidence reviewed"
        })),
    );
    expect_status(&reconsideration_review, 200, "Reconsideration review");
    let reconsideration_resolution = call(
        &application,
        "POST",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/reconsiderations/{reconsideration_id}/resolve"),
        Some(&keeper_token),
        Some(json!({
            "command": command("reconsider_resolve", 2),
            "reconsideration_id": reconsideration_id,
            "campaign_id": CAMPAIGN_ID,
            "resolution_event_id": "resolution_event_ar06_http",
            "outcome": "UPHELD",
            "resolution": "The original canonical decision stands",
            "corrected_event_type": null,
            "corrected_payload_json": null
        })),
    );
    expect_status(
        &reconsideration_resolution,
        200,
        "Reconsideration resolution",
    );

    let export_id = "export_ar06_http";
    let export = call(
        &application,
        "POST",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/exports"),
        Some(&keeper_token),
        Some(json!({
            "command": command("export_request", 0),
            "export_id": export_id,
            "campaign_id": CAMPAIGN_ID,
            "requested_by": KEEPER_ID,
            "audience": "CAMPAIGN_ARCHIVE",
            "requested_at_unix_ms": now + 6_000
        })),
    );
    expect_status(&export, 202, "Campaign export request");
    let export_query = call(
        &application,
        "GET",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/exports/{export_id}"),
        Some(&keeper_token),
        None,
    );
    expect_status(&export_query, 200, "Campaign export query");
    assert_eq!(export_query.body["state"], "REQUESTED");
    let hidden_export = call(
        &application,
        "GET",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/exports/{export_id}"),
        Some(&player_token),
        None,
    );
    expect_status(&hidden_export, 404, "keeper-only export opacity");

    let ended = call(
        &application,
        "PATCH",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/sessions/{SESSION_ID}"),
        Some(&keeper_token),
        Some(json!({
            "command": command("session_end", 2),
            "campaign_id": CAMPAIGN_ID,
            "session_id": SESSION_ID,
            "state": "ENDED",
            "changed_at_unix_ms": now + 7_000
        })),
    );
    expect_status(&ended, 200, "Session end");

    let child_campaign = call(
        &application,
        "POST",
        "/api/v1/campaigns",
        Some(&keeper_token),
        Some(json!({
            "command": command("child_campaign_create", 0),
            "campaign_id": CHILD_CAMPAIGN_ID,
            "owner_user_id": KEEPER_ID,
            "title": "AR06 fork child",
            "room_id": "room_ar06_http_fork",
            "room_name": "AR06 fork table",
            "created_at_unix_ms": 2,
            "authority": authority_body(CHILD_CAMPAIGN_ID)
        })),
    );
    expect_status(&child_campaign, 201, "Fork child Campaign create");
    let forked = call(
        &application,
        "POST",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/forks"),
        Some(&keeper_token),
        Some(json!({
            "command": command("campaign_fork", 0),
            "fork_id": "fork_ar06_http",
            "parent_campaign_id": CAMPAIGN_ID,
            "child_campaign_id": CHILD_CAMPAIGN_ID,
            "source_session_id": SESSION_ID,
            "reason": "Preserve a canonical branch"
        })),
    );
    expect_status(&forked, 201, "Campaign fork");

    let parent_for_player = call(
        &application,
        "GET",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}"),
        Some(&player_token),
        None,
    );
    expect_status(&parent_for_player, 200, "visible Campaign query");
    let child_for_player = call(
        &application,
        "GET",
        &format!("/api/v1/campaigns/{CHILD_CAMPAIGN_ID}"),
        Some(&player_token),
        None,
    );
    expect_status(&child_for_player, 404, "invisible fork query");

    let counts = setup_runtime
        .block_on(
            sqlx::query(
                r#"
                SELECT
                    (SELECT count(*) FROM public.event_store
                      WHERE event_type = 'CharacterUpdated'
                        AND campaign_id = $1) AS character_updates,
                    (SELECT count(*) FROM core_domain.session_characters
                      WHERE campaign_id = $1
                        AND character_id = $2) AS session_characters,
                    (SELECT count(*) FROM public.campaign_exports
                      WHERE campaign_id = $1
                        AND export_id = $3) AS exports,
                    (SELECT count(*) FROM public.campaign_forks
                      WHERE parent_campaign_id = $1
                        AND child_campaign_id = $4) AS forks
                "#,
            )
            .bind(CAMPAIGN_ID)
            .bind(CHARACTER_ID)
            .bind(export_id)
            .bind(CHILD_CAMPAIGN_ID)
            .fetch_one(&api_pool),
        )
        .expect("load AR06 lifecycle counts");
    assert_eq!(counts.get::<i64, _>("character_updates"), 1);
    assert_eq!(counts.get::<i64, _>("session_characters"), 1);
    assert_eq!(counts.get::<i64, _>("exports"), 1);
    assert_eq!(counts.get::<i64, _>("forks"), 1);

    let forged_projection = setup_runtime.block_on(
        sqlx::query(
            r#"
            INSERT INTO public.campaign_exports (
                export_id, campaign_id, requested_by, audience, state,
                requested_at, version, visibility_label, visibility_subject,
                provenance_kind, provenance_reference, provenance_recorded_by,
                last_event_sequence
            ) VALUES (
                'export_ar06_forged', $1, $2, 'CAMPAIGN_ARCHIVE', 'REQUESTED',
                now(), 1, 'keeper_only', 'not_applicable',
                'human_keeper_statement', 'forged', $2, $3
            )
            "#,
        )
        .bind(CAMPAIGN_ID)
        .bind(KEEPER_ID)
        .bind(action_sequence)
        .execute(&api_pool),
    );
    assert!(
        forged_projection.is_err(),
        "trpg_api_login must not forge a projection without canonical capability"
    );

    setup_runtime.block_on(async {
        api_pool.close().await;
        canonical_pool.close().await;
    });
    drop(application);
    let _ = std::fs::remove_file(&audit_path);
    let _ = std::fs::remove_file(audit_path.with_extension("jsonl.head"));
}
