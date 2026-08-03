    #[test]
    fn locked_human_keeper_authority_owner_can_manage_groups_but_players_cannot() {
        let mut service = service();
        for (user_id, login, role) in [
            ("group_server_owner", "group-server-owner@example.test", GlobalRole::ServerOwner),
            ("group_human_keeper", "group-human-keeper@example.test", GlobalRole::User),
            ("group_investigator", "group-investigator@example.test", GlobalRole::User),
        ] {
            service
                .create_user(user_id, login, "correct horse battery staple", role)
                .unwrap();
        }
        let owner_session = service
            .login(
                "group-server-owner@example.test",
                "correct horse battery staple",
                4_000,
            )
            .unwrap();
        let owner = service
            .authenticate_session(Some(owner_session.token.expose()), 4_001)
            .unwrap();
        for (user_id, role) in [
            ("group_human_keeper", CampaignRole::HumanKeeper),
            ("group_investigator", CampaignRole::Player),
        ] {
            service
                .grant_membership(&owner, "campaign_human_groups", user_id, role, 4_002)
                .unwrap();
        }
        service
            .register_authority_contract(
                &owner,
                AuthorityContract::new_locked(AuthorityContractDraft {
                    contract_id: "authority_campaign_human_groups".to_owned(),
                    campaign_id: "campaign_human_groups".to_owned(),
                    mode: AuthorityMode::HumanKp,
                    authority_owner: "group_human_keeper".to_owned(),
                    version: 1,
                    snapshot: AuthorityVersionSnapshotDraft {
                        ruleset_version: "coc7_rules_1".to_owned(),
                        house_rules_version: "house_rules_1".to_owned(),
                        scenario_version: "scenario_1".to_owned(),
                        prompt_version: "prompt_1".to_owned(),
                        agent_pack_version: "agent_pack_1".to_owned(),
                        tool_schema_version: "tool_schema_1".to_owned(),
                        safety_profile_version: "safety_profile_1".to_owned(),
                        ai_provider_snapshot: "provider_1".to_owned(),
                        model_route_snapshot: "route_1".to_owned(),
                        character_sheet_template_version: "character_1".to_owned(),
                    },
                    created_at_unix_ms: 4_002,
                })
                .unwrap(),
                4_002,
            )
            .unwrap();

        let keeper_session = service
            .login(
                "group-human-keeper@example.test",
                "correct horse battery staple",
                4_003,
            )
            .unwrap();
        let keeper = service
            .authenticate_session(Some(keeper_session.token.expose()), 4_004)
            .unwrap();
        service
            .create_campaign_group(
                &keeper,
                "campaign_human_groups",
                "investigation_red",
                4_005,
            )
            .unwrap();
        service
            .grant_group_membership(
                &keeper,
                "campaign_human_groups",
                "investigation_red",
                "group_investigator",
                4_005,
            )
            .unwrap();

        let player_session = service
            .login(
                "group-investigator@example.test",
                "correct horse battery staple",
                4_006,
            )
            .unwrap();
        let player = service
            .authenticate_session(Some(player_session.token.expose()), 4_007)
            .unwrap();
        assert_eq!(
            service.create_campaign_group(
                &player,
                "campaign_human_groups",
                "investigation_blue",
                4_008,
            ),
            Err(IdentityError::MembershipDenied)
        );
    }
