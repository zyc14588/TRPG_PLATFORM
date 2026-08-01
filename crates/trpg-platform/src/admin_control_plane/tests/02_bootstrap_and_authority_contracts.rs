#[test]
fn bootstrap_token_is_one_time_and_creates_exactly_two_distinct_roles() {
    let mut fixture = TestControl::new();
    let denied = fixture
        .control
        .handle(request(
            "GET",
            "/admin/v1/bootstrap/status",
            "wrong-bootstrap-token",
            None,
            Value::Null,
        ))
        .expect("denied response");
    assert_eq!(denied.status, 401);
    let owner_token = fixture.bootstrap();
    let replay = fixture
        .control
        .handle(request(
            "POST",
            "/admin/v1/bootstrap/complete",
            BOOTSTRAP_TOKEN,
            Some(0),
            json!({
                "administrator": {
                    "user_id": "server-owner-1",
                    "login": "owner@example.test",
                    "password": ADMIN_PASSWORD
                },
                "business_account": {
                    "user_id": "business-user-1",
                    "login": "business@example.test",
                    "password": BUSINESS_PASSWORD
                }
            }),
        ))
        .expect("replay response");
    assert_eq!(replay.status, 401);
    assert_eq!(replay.body["error"], "BOOTSTRAP_TOKEN_CONSUMED");
    let status = fixture
        .control
        .handle(request(
            "GET",
            "/admin/v1/bootstrap/status",
            &owner_token,
            None,
            Value::Null,
        ))
        .expect("status response");
    assert_eq!(status.status, 200);
    assert_eq!(status.body["administrator_count"], 1);
    assert_eq!(status.body["business_account_count"], 1);
}

#[test]
fn tutorial_authority_is_locked_to_the_business_keeper_and_replays() {
    let mut fixture = TestControl::new();
    let owner_token = fixture.bootstrap();
    let body = json!({
        "campaign_id": "tutorial_campaign",
        "contract_id": "tutorial_authority",
        "created_at_unix_ms": 1_700_000_000_000_u64,
        "ai_provider_snapshot": "tutorial_provider",
        "model_route_snapshot": "tutorial_route"
    });
    let created = fixture
        .control
        .handle(request(
            "POST",
            "/admin/v1/bootstrap/tutorial-authority",
            &owner_token,
            Some(1),
            body.clone(),
        ))
        .expect("tutorial authority response");
    assert_eq!(created.status, 201);
    assert_eq!(created.body["authority_mode"], "HUMAN_KP");
    assert_eq!(created.body["authority_owner"], "business-user-1");

    let campaign_id =
        trpg_shared_kernel::EntityId::new("tutorial_campaign").expect("campaign id");
    let contract = fixture
        .control
        .identity
        .authority_contract(&campaign_id)
        .expect("authority lookup")
        .expect("tutorial authority");
    assert!(contract.is_locked());
    assert_eq!(contract.change_policy(), trpg_shared_kernel::ChangePolicy::ForkOnly);
    assert_eq!(contract.authority_owner().as_str(), "business-user-1");
    assert_eq!(contract.snapshot().ruleset_version().as_str(), "coc7_rules_1");

    let replayed = fixture
        .control
        .handle(request(
            "POST",
            "/admin/v1/bootstrap/tutorial-authority",
            &owner_token,
            Some(2),
            body,
        ))
        .expect("tutorial authority replay");
    assert_eq!(replayed.status, 200);
    assert_eq!(replayed.body["replayed"], true);
    assert_eq!(replayed.body["state_version"], 2);
}

#[test]
fn server_owner_creates_users_and_forks_ai_authority_without_mutating_parent() {
    let mut fixture = TestControl::new();
    let owner_token = fixture.bootstrap();
    let tutorial = fixture
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
    assert_eq!(tutorial.status, 201);

    let user_body = json!({
        "user_id": "player-user-1",
        "login": "player@example.test",
        "password": "player-password-012345"
    });
    let created_user = fixture
        .control
        .handle(request(
            "POST",
            "/admin/v1/users",
            &owner_token,
            Some(2),
            user_body.clone(),
        ))
        .expect("managed user response");
    assert_eq!(created_user.status, 201);
    assert_eq!(created_user.body["global_role"], "USER");
    let replayed_user = fixture
        .control
        .handle(request(
            "POST",
            "/admin/v1/users",
            &owner_token,
            Some(3),
            user_body,
        ))
        .expect("managed user replay");
    assert_eq!(replayed_user.status, 200);
    assert_eq!(replayed_user.body["replayed"], true);

    let fork_body = json!({
        "parent_campaign_id": "tutorial_campaign",
        "child_campaign_id": "tutorial_campaign_ai",
        "authority_mode": "AI_KP",
        "authority_owner": "ai_keeper_tutorial",
        "campaign_manager_user_id": "player-user-1"
    });
    let forked = fixture
        .control
        .handle(request(
            "POST",
            "/admin/v1/authority-forks",
            &owner_token,
            Some(3),
            fork_body.clone(),
        ))
        .expect("authority fork response");
    assert_eq!(forked.status, 201);
    assert_eq!(forked.body["authority_mode"], "AI_KP");
    assert_eq!(forked.body["contract_id"], "authority_contract_tutorial_campaign_ai_1");

    let parent_id = EntityId::new("tutorial_campaign").expect("parent campaign id");
    let child_id = EntityId::new("tutorial_campaign_ai").expect("child campaign id");
    let parent = fixture
        .control
        .identity
        .authority_contract(&parent_id)
        .expect("parent lookup")
        .expect("parent authority");
    let child = fixture
        .control
        .identity
        .authority_contract(&child_id)
        .expect("child lookup")
        .expect("child authority");
    assert_eq!(parent.mode(), &AuthorityMode::HumanKp);
    assert_eq!(parent.authority_owner().as_str(), "business-user-1");
    assert_eq!(child.mode(), &AuthorityMode::AiKp);
    assert_eq!(child.authority_owner().as_str(), "ai_keeper_tutorial");
    assert_eq!(child.created_at_unix_ms(), parent.created_at_unix_ms() + 1);
    assert_eq!(child.snapshot(), parent.snapshot());

    let replayed_fork = fixture
        .control
        .handle(request(
            "POST",
            "/admin/v1/authority-forks",
            &owner_token,
            Some(4),
            fork_body,
        ))
        .expect("authority fork replay");
    assert_eq!(replayed_fork.status, 200);
    assert_eq!(replayed_fork.body["replayed"], true);

    let state = fs::read_to_string(fixture.root.join("state/admin.json"))
        .expect("read admin state");
    let audit = fs::read_to_string(fixture.root.join("audit/admin.jsonl"))
        .expect("read admin audit");
    assert!(!state.contains("player-password-012345"));
    assert!(!audit.contains("player-password-012345"));
}
