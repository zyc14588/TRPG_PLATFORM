
#[test]
fn refresh_rotates_session_and_logout_revokes_it() {
    let application = application();
    let (_, login) = exchange(
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
    let first = login["access_token"].as_str().unwrap().to_owned();
    let (status, refreshed) = exchange(
        application.clone(),
        json_request("POST", "/auth/refresh", Some(&first), None),
    );
    assert_eq!(status, 200);
    let second = refreshed["access_token"].as_str().unwrap().to_owned();
    assert_ne!(first, second);

    let (status, _) = exchange(
        application.clone(),
        json_request("GET", "/campaigns/campaign_a/authority", Some(&first), None),
    );
    assert_eq!(status, 401);
    let (status, _) = exchange(
        application.clone(),
        json_request("POST", "/auth/logout", Some(&second), None),
    );
    assert_eq!(status, 204);
    let (status, _) = exchange(
        application,
        json_request(
            "GET",
            "/campaigns/campaign_a/authority",
            Some(&second),
            None,
        ),
    );
    assert_eq!(status, 401);
}

#[test]
fn authorized_membership_mutation_requires_real_policy_and_is_audited() {
    let (application, audit_path) = governed_application()
        .expect("P02_OPENFGA_* and P02_OPA_ADDRESS must identify real policy services");
    let (status, login) = exchange(
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
    let owner_token = login["access_token"].as_str().unwrap();
    let (status, body) = exchange(
        application,
        json_request(
            "PUT",
            "/campaigns/campaign_a/memberships/player_a",
            Some(owner_token),
            Some(json!({"role": "SPECTATOR"})),
        ),
    );
    assert_eq!(status, 200, "unexpected policy response: {body}");
    assert_eq!(body["role"], "SPECTATOR");

    let records = FileAuditLog::open(&audit_path, "api-test-audit-v1", &AUDIT_KEY)
        .unwrap()
        .verify()
        .unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].decision, AuditDecision::Permit);
    assert_eq!(records[0].actor_id, "owner_a");
    assert_eq!(records[0].action, "manage_campaign_membership");
    assert_eq!(records[0].requested_role, "spectator");
    let mut anchor_name = audit_path.as_os_str().to_os_string();
    anchor_name.push(".head");
    std::fs::remove_file(audit_path).unwrap();
    std::fs::remove_file(PathBuf::from(anchor_name)).unwrap();
}
