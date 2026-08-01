    use super::*;

    const KEY: [u8; 32] = [7; 32];

    fn service() -> IdentityService {
        IdentityService::new(&KEY, 60_000).expect("valid identity service")
    }

    #[test]
    fn password_session_rotation_and_logout_are_enforced() {
        let mut service = service();
        service
            .create_user(
                "user_owner",
                "Owner@example.test",
                "correct horse battery staple",
                GlobalRole::ServerOwner,
            )
            .unwrap();
        assert_eq!(
            service.login("owner@example.test", "wrong password!!", 1_000),
            Err(IdentityError::InvalidCredentials)
        );

        let session = service
            .login("owner@example.test", "correct horse battery staple", 1_000)
            .unwrap();
        let context = service
            .authenticate_session(Some(session.token.expose()), 1_001)
            .unwrap();
        assert_eq!(context.subject_id().as_str(), "user_owner");

        let replacement = service
            .refresh_session(session.token.expose(), 2_000)
            .unwrap();
        assert_eq!(
            service.authenticate_session(Some(session.token.expose()), 2_001),
            Err(IdentityError::SessionRevoked)
        );
        service.logout(replacement.token.expose()).unwrap();
        assert_eq!(
            service.authenticate_session(Some(replacement.token.expose()), 2_002),
            Err(IdentityError::SessionRevoked)
        );
    }

    #[test]
    fn unknown_login_executes_the_dummy_argon2_verification_path() {
        let mut service = service();
        service.dummy_password_hash = "not-an-argon2-hash".to_owned();

        assert_eq!(
            service.login("missing@example.test", "incorrect password", 1_000),
            Err(IdentityError::PasswordHashFailure)
        );
        assert_eq!(
            service.login("!", "incorrect password", 1_001),
            Err(IdentityError::PasswordHashFailure)
        );
    }

    #[test]
    fn repeated_login_failures_are_rate_limited_without_blocking_later_recovery() {
        let mut service = service();
        service
            .create_user(
                "user_rate_limit",
                "rate-limit@example.test",
                "correct horse battery staple",
                GlobalRole::User,
            )
            .unwrap();
        for attempt in 0..LOGIN_FAILURE_LIMIT {
            assert_eq!(
                service.login(
                    "rate-limit@example.test",
                    "incorrect password",
                    1_000 + u64::from(attempt),
                ),
                Err(IdentityError::InvalidCredentials)
            );
        }
        assert_eq!(
            service.login(
                "rate-limit@example.test",
                "correct horse battery staple",
                1_010,
            ),
            Err(IdentityError::LoginRateLimited)
        );
        service
            .login(
                "rate-limit@example.test",
                "correct horse battery staple",
                61_010,
            )
            .unwrap();
    }

    #[test]
    fn plaintext_postgres_transport_is_restricted_to_local_endpoints() {
        assert!("postgresql://postgres@db.example.test/trpg"
            .parse::<PostgresConfig>()
            .unwrap()
            .get_hosts()
            .iter()
            .any(|host| matches!(host, PostgresHost::Tcp(value) if value == "db.example.test")));
        assert_eq!(
            connect_local_postgres("postgresql://postgres@db.example.test/trpg").err(),
            Some(IdentityError::PersistenceUnavailable)
        );
    }

    #[test]
    fn password_verification_gate_rejects_work_above_the_configured_bound() {
        let gate = PasswordVerificationGate::new(1).unwrap();
        let first = gate.try_acquire().unwrap();
        assert_eq!(
            gate.try_acquire().err(),
            Some(IdentityError::LoginRateLimited)
        );
        drop(first);
        assert!(gate.try_acquire().is_ok());
    }

    #[test]
    fn remote_postgres_requires_verified_tls_configuration() {
        let remote_without_tls = "postgresql://app@db.example.test/trpg?sslmode=disable"
            .parse::<PostgresConfig>()
            .unwrap();
        assert_eq!(
            uses_local_plaintext_postgres_transport(&remote_without_tls),
            Err(IdentityError::PersistenceUnavailable)
        );
        let remote_verified = "postgresql://app@db.example.test/trpg?sslmode=require"
            .parse::<PostgresConfig>()
            .unwrap();
        assert_eq!(
            uses_local_plaintext_postgres_transport(&remote_verified),
            Ok(false)
        );
        let local_plaintext = "postgresql://app@localhost/trpg"
            .parse::<PostgresConfig>()
            .unwrap();
        assert_eq!(
            uses_local_plaintext_postgres_transport(&local_plaintext),
            Ok(true)
        );
        let local_tls = "postgresql://app@localhost/trpg?sslmode=require"
            .parse::<PostgresConfig>()
            .unwrap();
        assert_eq!(
            uses_local_plaintext_postgres_transport(&local_tls),
            Ok(false)
        );
    }

    #[test]
    fn libpq_verify_full_url_is_normalized_only_with_explicit_ca_material() {
        let url = "postgresql://app@db.example.test/trpg?sslmode=verify-full&\
                   sslrootcert=%2Frun%2Fsecrets%2Fpostgres_ca_certificate";
        let config = parse_postgres_config(url, Some(b"certificate material")).unwrap();
        assert_eq!(config.get_ssl_mode(), PostgresSslMode::Require);
        assert!(config
            .get_hosts()
            .iter()
            .any(|host| matches!(host, PostgresHost::Tcp(value) if value == "db.example.test")));
        assert_eq!(
            parse_postgres_config(url, None).err(),
            Some(IdentityError::PersistenceUnavailable)
        );
    }

    #[test]
    fn verifier_rejects_a_context_after_its_session_is_logged_out() {
        let mut service = service();
        service
            .create_user(
                "user_owner",
                "owner@example.test",
                "correct horse battery staple",
                GlobalRole::ServerOwner,
            )
            .unwrap();
        let session = service
            .login("owner@example.test", "correct horse battery staple", 1_000)
            .unwrap();
        let context = service
            .authenticate_session(Some(session.token.expose()), 1_001)
            .unwrap();
        let verifier = service.verifier();
        verifier.verify(&context, 1_002).unwrap();

        service.logout(session.token.expose()).unwrap();

        assert_eq!(
            verifier.verify(&context, 1_003),
            Err(IdentityError::SessionRevoked)
        );
    }

    #[test]
    fn replay_authorization_is_campaign_bound_and_tracks_session_revocation() {
        let mut service = service();
        service
            .create_user(
                "replay_user",
                "replay-user@example.test",
                "correct horse battery staple",
                GlobalRole::ServerOwner,
            )
            .unwrap();
        let session = service
            .login(
                "replay-user@example.test",
                "correct horse battery staple",
                1_000,
            )
            .unwrap();
        let authentication = service
            .authenticate_session(Some(session.token.expose()), 1_001)
            .unwrap();
        service
            .grant_membership(
                &authentication,
                "campaign_replay_a",
                "replay_user",
                CampaignRole::Player,
                1_002,
            )
            .unwrap();
        let campaign_a = EntityId::new("campaign_replay_a").unwrap();
        let campaign_b = EntityId::new("campaign_replay_b").unwrap();
        let authorization = service
            .verifier()
            .authorize_replay(&authentication, &campaign_a, 1_003)
            .unwrap();
        let private = Visibility::private_to_player(EntityId::new("replay_user").unwrap());
        let private_group = Visibility::private_to_group(EntityId::new("unproven_group").unwrap());

        assert!(authorization
            .can_view(&campaign_a, &private, 1_004)
            .unwrap());
        assert!(!authorization
            .can_view(&campaign_a, &private_group, 1_004)
            .unwrap());
        service
            .create_campaign_group(
                &authentication,
                "campaign_replay_a",
                "unproven_group",
                1_004,
            )
            .unwrap();
        service
            .grant_group_membership(
                &authentication,
                "campaign_replay_a",
                "unproven_group",
                "replay_user",
                1_004,
            )
            .unwrap();
        assert!(authorization
            .can_view(&campaign_a, &private_group, 1_004)
            .unwrap());
        service
            .revoke_group_membership(
                &authentication,
                "campaign_replay_a",
                "unproven_group",
                "replay_user",
                1_004,
            )
            .unwrap();
        assert!(!authorization
            .can_view(&campaign_a, &private_group, 1_004)
            .unwrap());
        assert!(!authorization
            .can_view(&campaign_b, &private, 1_004)
            .unwrap());

        service.logout(session.token.expose()).unwrap();
        assert_eq!(
            authorization.can_view(&campaign_a, &private, 1_005),
            Err(IdentityError::SessionRevoked)
        );
    }
