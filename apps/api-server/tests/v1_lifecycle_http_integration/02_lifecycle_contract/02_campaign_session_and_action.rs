{

    let outsider_create = call(
        &application,
        "POST",
        "/api/v1/campaigns",
        Some(&outsider_token),
        Some(parent_campaign.clone()),
    );
    expect_status(&outsider_create, 403, "unauthorized Campaign create");
    let created = call(
        &application,
        "POST",
        "/api/v1/campaigns",
        Some(&keeper_token),
        Some(parent_campaign.clone()),
    );
    expect_status(&created, 201, "Campaign create");
    assert_eq!(created.body["aggregate_version"], 1);
    let created_retry = call(
        &application,
        "POST",
        "/api/v1/campaigns",
        Some(&keeper_token),
        Some(parent_campaign),
    );
    expect_status(&created_retry, 201, "Campaign create retry");
    assert_eq!(
        created_retry.body["last_event_sequence"],
        created.body["last_event_sequence"]
    );

    let outsider_list = call(
        &application,
        "GET",
        "/api/v1/campaigns",
        Some(&outsider_token),
        None,
    );
    expect_status(&outsider_list, 200, "membership-filtered Campaign list");
    assert_eq!(
        outsider_list.body["campaigns"]
            .as_array()
            .expect("Campaign array")
            .len(),
        0
    );
    let invisible_campaign = call(
        &application,
        "GET",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}"),
        Some(&outsider_token),
        None,
    );
    expect_status(&invisible_campaign, 404, "opaque invisible Campaign");
    assert_eq!(invisible_campaign.body["error"], "CORE_API_NOT_FOUND");

    let invite_body = json!({
        "command": command("invite_issue", 0),
        "campaign_id": CAMPAIGN_ID,
        "invite_id": "invite_ar06_http",
        "invited_user_id": PLAYER_ID,
        "role": "PLAYER",
        "expires_at_unix_ms": now + 600_000
    });
    let invite = call(
        &application,
        "POST",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/invites"),
        Some(&keeper_token),
        Some(invite_body.clone()),
    );
    expect_status(&invite, 201, "invite issue");
    let raw_token = invite.body["raw_token"]
        .as_str()
        .expect("invite raw token")
        .to_owned();
    let invite_retry = call(
        &application,
        "POST",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/invites"),
        Some(&keeper_token),
        Some(invite_body),
    );
    expect_status(&invite_retry, 201, "invite exact retry");
    assert!(
        invite_retry.body["raw_token"].as_str() == Some(raw_token.as_str()),
        "exact retry changed the issued invite token"
    );
    assert_eq!(
        invite_retry.body["last_event_sequence"],
        invite.body["last_event_sequence"]
    );
    let accepted = call(
        &application,
        "POST",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/invites/invite_ar06_http/accept"),
        Some(&player_token),
        Some(json!({
            "command": command("invite_accept", 1),
            "campaign_id": CAMPAIGN_ID,
            "invite_id": "invite_ar06_http",
            "accepting_user_id": PLAYER_ID,
            "raw_token": raw_token
        })),
    );
    expect_status(&accepted, 200, "invite acceptance");

    let create_character = call(
        &application,
        "POST",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/characters"),
        Some(&player_token),
        Some(json!({
            "command": command("character_create", 0),
            "campaign_id": CAMPAIGN_ID,
            "character_id": CHARACTER_ID,
            "owner_user_id": PLAYER_ID,
            "display_name": "Ada Mercer",
            "sheet_version_id": "sheet_ar06_http_v1",
            "sheet_json": character_sheet("Ada Mercer")
        })),
    );
    expect_status(&create_character, 201, "Character create");

    let update_body = json!({
        "command": command("character_update", 1),
        "campaign_id": CAMPAIGN_ID,
        "character_id": CHARACTER_ID,
        "owner_user_id": PLAYER_ID,
        "display_name": "Ada Mercer Updated",
        "sheet_version_id": "sheet_ar06_http_v2",
        "sheet_json": character_sheet("Ada Mercer Updated")
    });
    let updated = call(
        &application,
        "PUT",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/characters/{CHARACTER_ID}"),
        Some(&player_token),
        Some(update_body.clone()),
    );
    expect_status(&updated, 200, "Character update");
    assert_eq!(updated.body["aggregate_version"], 2);
    let update_retry = call(
        &application,
        "PUT",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/characters/{CHARACTER_ID}"),
        Some(&player_token),
        Some(update_body.clone()),
    );
    expect_status(&update_retry, 200, "Character update exact retry");
    assert_eq!(
        update_retry.body["last_event_sequence"],
        updated.body["last_event_sequence"]
    );
    let mut conflicting_reuse = update_body;
    conflicting_reuse["display_name"] = json!("Conflicting payload");
    let conflicting_reuse_response = call(
        &application,
        "PUT",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/characters/{CHARACTER_ID}"),
        Some(&player_token),
        Some(conflicting_reuse),
    );
    expect_status(
        &conflicting_reuse_response,
        409,
        "idempotency key payload conflict",
    );
    let stale_version = call(
        &application,
        "PUT",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/characters/{CHARACTER_ID}"),
        Some(&player_token),
        Some(json!({
            "command": command("character_stale", 1),
            "campaign_id": CAMPAIGN_ID,
            "character_id": CHARACTER_ID,
            "owner_user_id": PLAYER_ID,
            "display_name": "Stale update",
            "sheet_version_id": "sheet_ar06_http_stale",
            "sheet_json": character_sheet("Stale update")
        })),
    );
    expect_status(&stale_version, 409, "concurrent Character version conflict");

    let submitted = call(
        &application,
        "POST",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/characters/{CHARACTER_ID}/submit"),
        Some(&player_token),
        Some(json!({
            "command": command("character_submit", 2),
            "campaign_id": CAMPAIGN_ID,
            "character_id": CHARACTER_ID
        })),
    );
    expect_status(&submitted, 202, "Character submit");
    let reviewed = call(
        &application,
        "POST",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/characters/{CHARACTER_ID}/review"),
        Some(&keeper_token),
        Some(json!({
            "command": command("character_review", 3),
            "campaign_id": CAMPAIGN_ID,
            "character_id": CHARACTER_ID
        })),
    );
    expect_status(&reviewed, 200, "Character review");
    let current_sheet = setup_runtime
        .block_on(
            sqlx::query(
                "SELECT sheet.version, sheet.locked \
                   FROM public.characters AS character \
                   JOIN public.character_sheet_versions AS sheet \
                     ON sheet.character_id = character.character_id \
                    AND sheet.version = character.current_sheet_version \
                  WHERE character.character_id = $1",
            )
            .bind(CHARACTER_ID)
            .fetch_one(&api_pool),
        )
        .expect("load approved current Character sheet");
    assert_eq!(current_sheet.get::<i64, _>("version"), 2);
    assert!(current_sheet.get::<bool, _>("locked"));

    let tutorial = parse_scenario_yaml(include_str!(
        "../../../../../fixtures/scenarios/tutorial_mist_archive.scenario.yaml"
    ))
    .expect("parse AR06 tutorial scenario");
    let imported = call(
        &application,
        "POST",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/scenarios/import"),
        Some(&keeper_token),
        Some(json!({
            "command": command("scenario_import", 0),
            "campaign_id": CAMPAIGN_ID,
            "scenario_id": "scenario_ar06_http",
            "ruleset_id": tutorial.ruleset_id,
            "format_version": tutorial.format_version,
            "content_hash": tutorial.content_hash,
            "document_json": tutorial.canonical_json
        })),
    );
    expect_status(&imported, 201, "Scenario import");
    let started = call(
        &application,
        "POST",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/sessions"),
        Some(&keeper_token),
        Some(json!({
            "command": command("session_start", 0),
            "campaign_id": CAMPAIGN_ID,
            "session_id": SESSION_ID,
            "room_id": "room_ar06_http",
            "scenario_id": "scenario_ar06_http",
            "scene_id": "scene_ar06_archive",
            "scene_key": "scene_archive_front",
            "scene_name": "Archive front",
            "started_at_unix_ms": now + 1_000
        })),
    );
    expect_status(&started, 201, "Session start");
    let joined = call(
        &application,
        "POST",
        &format!(
            "/api/v1/campaigns/{CAMPAIGN_ID}/sessions/{SESSION_ID}/characters/{CHARACTER_ID}/join"
        ),
        Some(&player_token),
        Some(json!({
            "command": command("character_join", 0),
            "join_id": "join_ar06_http",
            "campaign_id": CAMPAIGN_ID,
            "session_id": SESSION_ID,
            "character_id": CHARACTER_ID,
            "owner_user_id": PLAYER_ID,
            "joined_at_unix_ms": now + 2_000
        })),
    );
    expect_status(&joined, 201, "Character join Session");
    let switched = call(
        &application,
        "POST",
        &format!("/api/v1/campaigns/{CAMPAIGN_ID}/sessions/{SESSION_ID}/scenes"),
        Some(&keeper_token),
        Some(json!({
            "command": command("scene_switch", 1),
            "campaign_id": CAMPAIGN_ID,
            "session_id": SESSION_ID,
            "next_scene_id": "scene_ar06_basement",
            "next_scene_key": "scene_basement",
            "next_scene_name": "Basement",
            "switched_at_unix_ms": now + 3_000
        })),
    );
    expect_status(&switched, 201, "Scene switch");
    include!("03_action_reconsideration_fork_and_export.rs");
}
