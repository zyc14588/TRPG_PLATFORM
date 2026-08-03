    #[test]
    fn player_cannot_self_grant_private_group_access() {
        let mut service = service();
        service
            .create_user(
                "group_owner",
                "group-owner@example.test",
                "correct horse battery staple",
                GlobalRole::ServerOwner,
            )
            .unwrap();
        service
            .create_user(
                "group_player",
                "group-player@example.test",
                "another correct horse battery",
                GlobalRole::User,
            )
            .unwrap();
        let owner_session = service
            .login(
                "group-owner@example.test",
                "correct horse battery staple",
                3_000,
            )
            .unwrap();
        let owner = service
            .authenticate_session(Some(owner_session.token.expose()), 3_001)
            .unwrap();
        service
            .grant_membership(
                &owner,
                "campaign_group_access",
                "group_player",
                CampaignRole::Player,
                3_002,
            )
            .unwrap();
        service
            .create_campaign_group(
                &owner,
                "campaign_group_access",
                "investigation_alpha",
                3_002,
            )
            .unwrap();
        let player_session = service
            .login(
                "group-player@example.test",
                "another correct horse battery",
                3_003,
            )
            .unwrap();
        let player = service
            .authenticate_session(Some(player_session.token.expose()), 3_004)
            .unwrap();

        assert_eq!(
            service.grant_group_membership(
                &player,
                "campaign_group_access",
                "investigation_alpha",
                "group_player",
                3_005,
            ),
            Err(IdentityError::MembershipDenied)
        );
    }
