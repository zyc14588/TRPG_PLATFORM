#[test]
fn http_authentication_and_campaign_authorization_fail_closed() {
    let application = application();
    let (status, _) = exchange(
        application.clone(),
        json_request("GET", "/campaigns/campaign_a/authority", None, None),
    );
    assert_eq!(status, 401);

    let (status, body) = exchange(
        application.clone(),
        json_request(
            "POST",
            "/auth/login",
            None,
            Some(json!({
                "login": "player@example.test",
                "password": "player password long enough",
                "role": "SERVER_OWNER"
            })),
        ),
    );
    assert_eq!(status, 400);
    assert_eq!(body["error"], "INVALID_JSON_BODY");

    let (status, body) = exchange(
        application.clone(),
        json_request(
            "POST",
            "/auth/login",
            None,
            Some(json!({
                "login": "player@example.test",
                "password": "player password long enough"
            })),
        ),
    );
    assert_eq!(status, 200);
    assert_eq!(body["user_id"], "player_a");
    assert_eq!(body["global_role"], "USER");
    let token = body["access_token"].as_str().unwrap().to_owned();

    let (status, _) = exchange(
        application.clone(),
        json_request("GET", "/campaigns/campaign_b/authority", Some(&token), None),
    );
    assert_eq!(status, 403);
    let (status, _) = exchange(
        application.clone(),
        json_request(
            "PUT",
            "/campaigns/campaign_a/memberships/player_a",
            Some(&token),
            Some(json!({"role": "CAMPAIGN_OWNER"})),
        ),
    );
    assert_eq!(status, 403);

    let (status, owner_login) = exchange(
        application.clone(),
        json_request(
            "POST",
            "/auth/login",
            None,
            Some(json!({
                "login": "owner@example.test",
                "password": "owner password long enough"
            })),
        ),
    );
    assert_eq!(status, 200);
    assert_eq!(owner_login["user_id"], "owner_a");
    assert_eq!(owner_login["global_role"], "SERVER_OWNER");
    let owner_token = owner_login["access_token"].as_str().unwrap();

    let (status, body) = exchange(
        application.clone(),
        json_request(
            "GET",
            "/campaigns/campaign_a/membership",
            Some(&token),
            None,
        ),
    );
    assert_eq!(status, 200);
    assert_eq!(body["role"], "PLAYER");

    let (status, body) = exchange(
        application.clone(),
        json_request(
            "POST",
            "/campaigns/campaign_a/groups/red_team",
            Some(owner_token),
            None,
        ),
    );
    assert_eq!(status, 201);
    assert_eq!(body["group_id"], "red_team");
    let (status, body) = exchange(
        application.clone(),
        json_request(
            "PUT",
            "/campaigns/campaign_a/groups/red_team/memberships/player_a",
            Some(owner_token),
            None,
        ),
    );
    assert_eq!(status, 200);
    assert_eq!(body["user_id"], "player_a");

    let (status, body) = exchange(
        application.clone(),
        json_request(
            "POST",
            "/campaigns/campaign_a/groups/player_self_grant",
            Some(&token),
            None,
        ),
    );
    assert_eq!(status, 403);
    assert_eq!(body["error"], "CAMPAIGN_MEMBERSHIP_DENIED");
    let (status, body) = exchange(
        application.clone(),
        json_request(
            "PUT",
            "/campaigns/campaign_a/memberships/player_a",
            Some(owner_token),
            Some(json!({"role": "SPECTATOR"})),
        ),
    );
    assert_eq!(status, 503);
    assert_eq!(body["error"], "POLICY_UNAVAILABLE");

    let (status, body) = exchange(
        application,
        json_request("GET", "/campaigns/campaign_a/authority", Some(&token), None),
    );
    assert_eq!(status, 200);
    assert_eq!(body["authority_owner"], "owner_a");
    assert_eq!(body["change_policy"], "FORK_ONLY");
    assert_eq!(body["created_at_unix_ms"], 1);
    assert_eq!(body["snapshot"]["ruleset_version"], "coc7_rules_1");
}
