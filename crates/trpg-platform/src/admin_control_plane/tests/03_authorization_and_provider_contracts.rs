#[test]
fn authority_fork_rejects_in_place_or_unprivileged_requests() {
    let mut fixture = TestControl::new();
    let owner_token = fixture.bootstrap();
    fixture
        .control
        .handle(request(
            "POST",
            "/admin/v1/bootstrap/tutorial-authority",
            &owner_token,
            Some(1),
            json!({
                "campaign_id": "tutorial_campaign",
                "contract_id": "tutorial_authority",
                "created_at_unix_ms": 1_700_000_000_000_u64,
                "ai_provider_snapshot": "tutorial_provider",
                "model_route_snapshot": "tutorial_route"
            }),
        ))
        .expect("tutorial authority response");
    let business_token = fixture
        .control
        .identity
        .login(
            "business@example.test",
            BUSINESS_PASSWORD,
            now_unix_ms().expect("test time"),
        )
        .expect("business login")
        .token
        .expose()
        .to_owned();
    let request_body = json!({
        "parent_campaign_id": "tutorial_campaign",
        "child_campaign_id": "tutorial_campaign",
        "authority_mode": "AI_KP",
        "authority_owner": "ai_keeper_tutorial",
        "campaign_manager_user_id": "business-user-1"
    });
    let denied = fixture
        .control
        .handle(request(
            "POST",
            "/admin/v1/authority-forks",
            &business_token,
            Some(2),
            request_body.clone(),
        ))
        .expect("unprivileged authority fork response");
    assert_eq!(denied.status, 403);
    let in_place = fixture
        .control
        .handle(request(
            "POST",
            "/admin/v1/authority-forks",
            &owner_token,
            Some(2),
            request_body,
        ))
        .expect("in-place authority fork response");
    assert_eq!(in_place.status, 400);
    assert_eq!(in_place.body["error"], "AUTHORITY_FORK_REQUEST_INVALID");
}

#[test]
fn business_user_cannot_use_admin_operations() {
    let mut fixture = TestControl::new();
    fixture.bootstrap();
    let business_token = fixture
        .control
        .identity
        .login(
            "business@example.test",
            BUSINESS_PASSWORD,
            now_unix_ms().expect("test time"),
        )
        .expect("business login")
        .token
        .expose()
        .to_owned();
    let response = fixture
        .control
        .handle(request(
            "GET",
            "/admin/v1/diagnostics",
            &business_token,
            None,
            Value::Null,
        ))
        .expect("diagnostics response");
    assert_eq!(response.status, 403);
    assert_eq!(response.body["error"], "ADMIN_SERVER_OWNER_REQUIRED");
}

#[test]
fn provider_secret_is_reference_only_and_probe_is_idempotent() {
    let mut fixture = TestControl::new();
    let owner_token = fixture.bootstrap();
    let rejected = fixture
        .control
        .handle(request(
            "PUT",
            "/admin/v1/providers/configuration",
            &owner_token,
            Some(1),
            json!({
                "provider_type": "openai",
                "base_url": "https://provider.example.test/v1",
                "model_id": "model-1",
                "model_artifact_sha256": format!("sha256:{}", "a".repeat(64)),
                "credential_secret_id": "provider_credential",
                "credential_secret_version": 1,
                "api_key": PROVIDER_CANARY
            }),
        ))
        .expect("rejected response");
    assert_eq!(rejected.status, 400);
    let configured = fixture
        .control
        .handle(request(
            "PUT",
            "/admin/v1/providers/configuration",
            &owner_token,
            Some(1),
            json!({
                "provider_type": "openai",
                "base_url": "https://provider.example.test/v1",
                "model_id": "model-1",
                "model_artifact_sha256": format!("sha256:{}", "a".repeat(64)),
                "credential_secret_id": "provider_credential",
                "credential_secret_version": 1
            }),
        ))
        .expect("configure response");
    assert_eq!(configured.status, 200);
    let probed = fixture
        .control
        .handle(request(
            "POST",
            "/admin/v1/providers/probe",
            &owner_token,
            Some(2),
            Value::Null,
        ))
        .expect("probe response");
    assert_eq!(probed.body["result"], "PROVIDER_REACHABLE");
    let replayed = fixture
        .control
        .handle(request_with_key(
            "POST",
            "/admin/v1/providers/probe",
            &owner_token,
            Some(2),
            "key-POST-admin-v1-providers-probe",
            Value::Null,
        ))
        .expect("replayed probe");
    assert_eq!(replayed.body["replayed"], true);
    assert_eq!(
        replayed.body["artifact_reference"],
        probed.body["artifact_reference"]
    );
    assert_eq!(replayed.body["digest"], probed.body["digest"]);
    let persisted = read_tree_text(&fixture.root);
    assert!(!persisted.contains(PROVIDER_CANARY));
}
