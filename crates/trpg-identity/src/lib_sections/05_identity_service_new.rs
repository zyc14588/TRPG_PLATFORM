
impl IdentityService {
    pub fn new(signing_key: &[u8], session_ttl_ms: u64) -> Result<Self, IdentityError> {
        if signing_key.len() != SIGNING_KEY_BYTES || session_ttl_ms == 0 {
            return Err(IdentityError::InvalidSigningKey);
        }
        let mut key = [0_u8; SIGNING_KEY_BYTES];
        key.copy_from_slice(signing_key);
        let dummy_password_hash = Argon2::default()
            .hash_password(DUMMY_PASSWORD.as_bytes(), &SaltString::generate(&mut OsRng))
            .map_err(|_| IdentityError::PasswordHashFailure)?
            .to_string();
        Ok(Self {
            users_by_login: HashMap::new(),
            users_by_id: HashMap::new(),
            sessions_by_hash: HashMap::new(),
            memberships: HashMap::new(),
            campaign_groups: HashMap::new(),
            group_memberships: HashMap::new(),
            authorities: HashMap::new(),
            database: None,
            persistent_verification: None,
            verification_state: Arc::new(RwLock::new(VerificationState::default())),
            signing_key: key,
            session_ttl_ms,
            dummy_password_hash,
            login_attempts: HashMap::new(),
            distributed_login_security: None,
            password_verification_gate: PasswordVerificationGate::new(DEFAULT_ARGON2_CONCURRENCY)?,
        })
    }

    pub fn from_postgres(
        database_url: &str,
        signing_key: &[u8],
        session_ttl_ms: u64,
    ) -> Result<Self, IdentityError> {
        if database_url.trim().is_empty() {
            return Err(IdentityError::PersistenceUnavailable);
        }
        let mut client = connect_postgres(database_url, None)?;
        apply_identity_migrations(&mut client)?;
        let persistent_verification =
            PersistentVerificationStore::new(connect_postgres(database_url, None)?);
        let mut service = Self::new(signing_key, session_ttl_ms)?;
        service.database = Some(client);
        service.persistent_verification = Some(persistent_verification);
        service.reload_from_database()?;
        Ok(service)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_postgres_with_distributed_login_security(
        database_url: &str,
        redis_url: &str,
        redis_namespace: &str,
        signing_key: &[u8],
        session_ttl_ms: u64,
        argon2_concurrency: usize,
    ) -> Result<Self, IdentityError> {
        Self::from_postgres_with_security(
            database_url,
            None,
            redis_url,
            redis_namespace,
            signing_key,
            session_ttl_ms,
            argon2_concurrency,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_postgres_with_security(
        database_url: &str,
        postgres_ca_certificate_pem: Option<&[u8]>,
        redis_url: &str,
        redis_namespace: &str,
        signing_key: &[u8],
        session_ttl_ms: u64,
        argon2_concurrency: usize,
    ) -> Result<Self, IdentityError> {
        Self::from_postgres_with_security_and_redis_tls(
            database_url,
            postgres_ca_certificate_pem,
            redis_url,
            redis_namespace,
            signing_key,
            session_ttl_ms,
            argon2_concurrency,
            None,
            None,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_postgres_with_security_and_redis_tls(
        database_url: &str,
        postgres_ca_certificate_pem: Option<&[u8]>,
        redis_url: &str,
        redis_namespace: &str,
        signing_key: &[u8],
        session_ttl_ms: u64,
        argon2_concurrency: usize,
        redis_root_certificate: Option<&[u8]>,
        redis_client_certificate: Option<&[u8]>,
        redis_client_private_key: Option<&[u8]>,
    ) -> Result<Self, IdentityError> {
        Self::initialize_postgres_with_security_and_redis_tls(
            database_url,
            postgres_ca_certificate_pem,
            redis_url,
            redis_namespace,
            signing_key,
            session_ttl_ms,
            argon2_concurrency,
            redis_root_certificate,
            redis_client_certificate,
            redis_client_private_key,
            true,
        )
    }

    /// Connects the production identity service to a schema prepared by the
    /// dedicated migration runner. Missing or unreadable schema objects still
    /// fail closed during the initial database reload.
    #[allow(clippy::too_many_arguments)]
    pub fn from_prepared_postgres_with_security_and_redis_tls(
        database_url: &str,
        postgres_ca_certificate_pem: Option<&[u8]>,
        redis_url: &str,
        redis_namespace: &str,
        signing_key: &[u8],
        session_ttl_ms: u64,
        argon2_concurrency: usize,
        redis_root_certificate: Option<&[u8]>,
        redis_client_certificate: Option<&[u8]>,
        redis_client_private_key: Option<&[u8]>,
    ) -> Result<Self, IdentityError> {
        Self::initialize_postgres_with_security_and_redis_tls(
            database_url,
            postgres_ca_certificate_pem,
            redis_url,
            redis_namespace,
            signing_key,
            session_ttl_ms,
            argon2_concurrency,
            redis_root_certificate,
            redis_client_certificate,
            redis_client_private_key,
            false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn initialize_postgres_with_security_and_redis_tls(
        database_url: &str,
        postgres_ca_certificate_pem: Option<&[u8]>,
        redis_url: &str,
        redis_namespace: &str,
        signing_key: &[u8],
        session_ttl_ms: u64,
        argon2_concurrency: usize,
        redis_root_certificate: Option<&[u8]>,
        redis_client_certificate: Option<&[u8]>,
        redis_client_private_key: Option<&[u8]>,
        apply_migrations: bool,
    ) -> Result<Self, IdentityError> {
        if database_url.trim().is_empty() {
            return Err(IdentityError::PersistenceUnavailable);
        }
        let distributed_login_security = DistributedLoginSecurity::connect_with_tls(
            redis_url,
            redis_namespace,
            redis_root_certificate,
            redis_client_certificate,
            redis_client_private_key,
        )?;
        let mut client = connect_postgres(database_url, postgres_ca_certificate_pem)?;
        if apply_migrations {
            apply_identity_migrations(&mut client)?;
        }
        let persistent_verification = PersistentVerificationStore::new(connect_postgres(
            database_url,
            postgres_ca_certificate_pem,
        )?);
        let mut service = Self::new(signing_key, session_ttl_ms)?;
        service.database = Some(client);
        service.persistent_verification = Some(persistent_verification);
        service.distributed_login_security = Some(distributed_login_security);
        service.password_verification_gate = PasswordVerificationGate::new(argon2_concurrency)?;
        service.reload_from_database()?;
        Ok(service)
    }

    pub const fn is_persistent(&self) -> bool {
        self.database.is_some()
    }

    pub const fn is_distributed_login_protected(&self) -> bool {
        self.distributed_login_security.is_some()
    }

    pub fn check_readiness(&mut self) -> Result<(), IdentityError> {
        let database = self
            .database
            .as_mut()
            .ok_or(IdentityError::PersistenceUnavailable)?;
        database
            .check_connection()
            .map_err(|_| IdentityError::PersistenceUnavailable)?;
        self.persistent_verification
            .as_ref()
            .ok_or(IdentityError::PersistenceUnavailable)?
            .check_readiness()?;
        match &self.distributed_login_security {
            Some(security) => security.check_readiness(),
            None => Ok(()),
        }
    }

    pub fn verifier(&self) -> IdentityVerifier {
        IdentityVerifier {
            issuer_fingerprint: Sha256::digest(self.signing_key).into(),
            state: Arc::clone(&self.verification_state),
            persistent_verification: self.persistent_verification.clone(),
        }
    }

    fn publish_verification_state(&self) -> Result<(), IdentityError> {
        let sessions_by_id = self
            .sessions_by_hash
            .values()
            .map(|session| {
                (
                    session.session_id.clone(),
                    SessionVerificationRecord {
                        user_id: session.user_id.clone(),
                        issued_at_unix_ms: session.issued_at_unix_ms,
                        expires_at_unix_ms: session.expires_at_unix_ms,
                        revoked: session.revoked,
                    },
                )
            })
            .collect();
        let mut state = self
            .verification_state
            .write()
            .map_err(|_| IdentityError::PersistenceUnavailable)?;
        state.sessions_by_id = sessions_by_id;
        state.memberships = self
            .memberships
            .iter()
            .map(|(key, membership)| (key.clone(), membership.role))
            .collect();
        state.group_memberships = self.group_memberships.keys().cloned().collect();
        state.authorities = self.authorities.clone();
        Ok(())
    }
}
