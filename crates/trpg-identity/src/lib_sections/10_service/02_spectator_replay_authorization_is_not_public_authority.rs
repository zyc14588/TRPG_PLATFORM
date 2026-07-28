
    #[test]
    fn spectator_replay_authorization_is_not_public_authority() {
        let mut service = service();
        service
            .create_user(
                "spectator_user",
                "spectator-user@example.test",
                "correct horse battery staple",
                GlobalRole::ServerOwner,
            )
            .unwrap();
        let session = service
            .login(
                "spectator-user@example.test",
                "correct horse battery staple",
                2_000,
            )
            .unwrap();
        let authentication = service
            .authenticate_session(Some(session.token.expose()), 2_001)
            .unwrap();
        service
            .grant_membership(
                &authentication,
                "campaign_spectator",
                "spectator_user",
                CampaignRole::Spectator,
                2_002,
            )
            .unwrap();
        let campaign = EntityId::new("campaign_spectator").unwrap();
        let authorization = service
            .verifier()
            .authorize_replay(&authentication, &campaign, 2_003)
            .unwrap();

        assert!(authorization
            .can_view(
                &campaign,
                &Visibility::new(trpg_shared_kernel::VisibilityLabel::SpectatorVisible),
                2_004,
            )
            .unwrap());
        assert!(!authorization
            .can_view(
                &campaign,
                &Visibility::new(trpg_shared_kernel::VisibilityLabel::SpectatorHidden),
                2_004,
            )
            .unwrap());
        assert!(!authorization
            .can_view(
                &campaign,
                &Visibility::new(trpg_shared_kernel::VisibilityLabel::PartyVisible),
                2_004,
            )
            .unwrap());
    }

    #[test]
    fn logged_out_context_cannot_manage_memberships() {
        let mut service = service();
        service
            .create_user(
                "user_owner",
                "owner@example.test",
                "correct horse battery staple",
                GlobalRole::ServerOwner,
            )
            .unwrap();
        service
            .create_user(
                "user_player",
                "player@example.test",
                "another correct horse battery",
                GlobalRole::User,
            )
            .unwrap();
        let session = service
            .login("owner@example.test", "correct horse battery staple", 1_000)
            .unwrap();
        let context = service
            .authenticate_session(Some(session.token.expose()), 1_001)
            .unwrap();
        service.logout(session.token.expose()).unwrap();

        assert_eq!(
            service.grant_membership(
                &context,
                "campaign_a",
                "user_player",
                CampaignRole::Player,
                1_002,
            ),
            Err(IdentityError::SessionRevoked)
        );
    }

    #[test]
    fn campaign_owner_cannot_grant_privileged_roles_and_human_keeper_is_unique() {
        let mut service = service();
        for (id, login, role) in [
            (
                "server_owner",
                "server@example.test",
                GlobalRole::ServerOwner,
            ),
            ("campaign_owner", "campaign@example.test", GlobalRole::User),
            ("keeper_a", "keeper-a@example.test", GlobalRole::User),
            ("keeper_b", "keeper-b@example.test", GlobalRole::User),
        ] {
            service
                .create_user(id, login, "correct horse battery staple", role)
                .unwrap();
        }
        let server_session = service
            .login("server@example.test", "correct horse battery staple", 1_000)
            .unwrap();
        let server = service
            .authenticate_session(Some(server_session.token.expose()), 1_001)
            .unwrap();
        service
            .grant_membership(
                &server,
                "campaign_a",
                "campaign_owner",
                CampaignRole::CampaignOwner,
                1_002,
            )
            .unwrap();
        service
            .grant_membership(
                &server,
                "campaign_a",
                "keeper_a",
                CampaignRole::HumanKeeper,
                1_002,
            )
            .unwrap();
        assert_eq!(
            service.grant_membership(
                &server,
                "campaign_a",
                "keeper_b",
                CampaignRole::HumanKeeper,
                1_003,
            ),
            Err(IdentityError::MembershipDenied)
        );

        let owner_session = service
            .login(
                "campaign@example.test",
                "correct horse battery staple",
                1_000,
            )
            .unwrap();
        let owner = service
            .authenticate_session(Some(owner_session.token.expose()), 1_001)
            .unwrap();
        for privileged_role in [CampaignRole::CampaignOwner, CampaignRole::HumanKeeper] {
            assert_eq!(
                service.grant_membership(&owner, "campaign_a", "keeper_b", privileged_role, 1_003,),
                Err(IdentityError::MembershipDenied)
            );
        }
        service
            .grant_membership(
                &owner,
                "campaign_a",
                "keeper_b",
                CampaignRole::Player,
                1_003,
            )
            .unwrap();
    }

    #[test]
    fn membership_is_resource_scoped_and_fail_closed() {
        let mut service = service();
        service
            .create_user(
                "user_owner",
                "owner@example.test",
                "correct horse battery staple",
                GlobalRole::ServerOwner,
            )
            .unwrap();
        service
            .create_user(
                "user_player",
                "player@example.test",
                "another correct horse battery",
                GlobalRole::User,
            )
            .unwrap();
        let owner_session = service
            .login("owner@example.test", "correct horse battery staple", 1_000)
            .unwrap();
        let owner = service
            .authenticate_session(Some(owner_session.token.expose()), 1_001)
            .unwrap();
        service
            .grant_membership(
                &owner,
                "campaign_a",
                "user_player",
                CampaignRole::Player,
                1_002,
            )
            .unwrap();

        let player_session = service
            .login(
                "player@example.test",
                "another correct horse battery",
                1_000,
            )
            .unwrap();
        let player = service
            .authenticate_session(Some(player_session.token.expose()), 1_001)
            .unwrap();
        service
            .require_membership(
                &player,
                &EntityId::new("campaign_a").unwrap(),
                &[CampaignRole::Player],
                1_002,
            )
            .unwrap();
        let campaign_a = EntityId::new("campaign_a").unwrap();
        let command_actor = service.command_actor(&player, &campaign_a, 1_002).unwrap();
        assert_eq!(command_actor.role(), &ActorRole::Investigator);
        assert_eq!(
            service.require_membership(
                &player,
                &EntityId::new("campaign_b").unwrap(),
                &[CampaignRole::Player],
                1_002,
            ),
            Err(IdentityError::MembershipRequired)
        );
        assert_eq!(
            service.command_actor(&player, &EntityId::new("campaign_b").unwrap(), 1_002),
            Err(IdentityError::MembershipRequired)
        );
    }

    #[test]
    fn forged_workload_and_cross_campaign_agent_tokens_are_rejected() {
        let service = service();
        let workload = service
            .issue_workload_credential("workflow_1", WorkloadRole::WorkflowEngine, 1_000, 2_000)
            .unwrap();
        let mut forged = workload.clone();
        forged.replace_range(3..4, "x");
        assert_eq!(
            service.authenticate_workload(&forged, 1_500),
            Err(IdentityError::InvalidInternalCredential)
        );
        assert!(service.authenticate_workload(&workload, 1_500).is_ok());
        let workload_context = service.authenticate_workload(&workload, 1_500).unwrap();
        let campaign = EntityId::new("campaign_a").unwrap();
        let verifier = service.verifier();
        verifier
            .verify_actor(
                &workload_context,
                &Actor::verified_workload("workflow_1", KernelWorkloadRole::WorkflowEngine)
                    .unwrap(),
                &campaign,
                1_500,
            )
            .unwrap();
        assert_eq!(
            verifier.verify_actor(
                &workload_context,
                &Actor::verified_workload("workflow_1", KernelWorkloadRole::RulesEngine).unwrap(),
                &campaign,
                1_500,
            ),
            Err(IdentityError::InvalidInternalCredential)
        );

        let agent = service
            .issue_agent_run_credential(
                "run_1",
                "agent_keeper",
                "campaign_a",
                AgentClass::AiKeeperOrchestrator,
                1_000,
                2_000,
            )
            .unwrap();
        let context = service.authenticate_agent_run(&agent, 1_500).unwrap();
        assert_eq!(
            context.require_campaign(&EntityId::new("campaign_b").unwrap()),
            Err(IdentityError::CampaignScopeMismatch)
        );
    }
