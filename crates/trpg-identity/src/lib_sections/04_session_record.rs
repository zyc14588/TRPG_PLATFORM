
#[derive(Clone, Debug)]
struct SessionRecord {
    session_id: EntityId,
    user_id: EntityId,
    issued_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    revoked: bool,
}

#[derive(Clone, Debug)]
struct SessionVerificationRecord {
    user_id: EntityId,
    issued_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    revoked: bool,
}

#[derive(Debug, Default)]
struct VerificationState {
    sessions_by_id: HashMap<EntityId, SessionVerificationRecord>,
    memberships: HashMap<(EntityId, EntityId), CampaignRole>,
    group_memberships: HashSet<(EntityId, EntityId, EntityId)>,
    authorities: HashMap<EntityId, AuthorityContract>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CampaignMembership {
    campaign_id: EntityId,
    user_id: EntityId,
    role: CampaignRole,
}

impl CampaignMembership {
    pub fn campaign_id(&self) -> &EntityId {
        &self.campaign_id
    }

    pub fn user_id(&self) -> &EntityId {
        &self.user_id
    }

    pub const fn role(&self) -> CampaignRole {
        self.role
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CampaignGroup {
    campaign_id: EntityId,
    group_id: EntityId,
}

impl CampaignGroup {
    pub fn campaign_id(&self) -> &EntityId {
        &self.campaign_id
    }

    pub fn group_id(&self) -> &EntityId {
        &self.group_id
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CampaignGroupMembership {
    campaign_id: EntityId,
    group_id: EntityId,
    user_id: EntityId,
}

impl CampaignGroupMembership {
    pub fn campaign_id(&self) -> &EntityId {
        &self.campaign_id
    }

    pub fn group_id(&self) -> &EntityId {
        &self.group_id
    }

    pub fn user_id(&self) -> &EntityId {
        &self.user_id
    }
}

pub struct IdentityService {
    users_by_login: HashMap<String, UserRecord>,
    users_by_id: HashMap<EntityId, UserRecord>,
    sessions_by_hash: HashMap<[u8; 32], SessionRecord>,
    memberships: HashMap<(EntityId, EntityId), CampaignMembership>,
    campaign_groups: HashMap<(EntityId, EntityId), CampaignGroup>,
    group_memberships: HashMap<(EntityId, EntityId, EntityId), CampaignGroupMembership>,
    authorities: HashMap<EntityId, AuthorityContract>,
    database: Option<Client>,
    persistent_verification: Option<PersistentVerificationStore>,
    verification_state: Arc<RwLock<VerificationState>>,
    signing_key: [u8; SIGNING_KEY_BYTES],
    session_ttl_ms: u64,
    dummy_password_hash: String,
    login_attempts: HashMap<String, LoginAttemptState>,
    distributed_login_security: Option<DistributedLoginSecurity>,
    password_verification_gate: PasswordVerificationGate,
}

#[derive(Clone, Copy, Debug)]
struct LoginAttemptState {
    window_started_at_unix_ms: u64,
    failures: u32,
    blocked_until_unix_ms: u64,
}

#[derive(Clone)]
struct DistributedLoginSecurity {
    client: redis::Client,
    namespace: String,
}

impl fmt::Debug for DistributedLoginSecurity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DistributedLoginSecurity")
            .field("client", &"[REDIS CLIENT]")
            .field("namespace", &self.namespace)
            .finish()
    }
}

impl DistributedLoginSecurity {
    fn connect_with_tls(
        redis_url: &str,
        namespace: &str,
        root_certificate: Option<&[u8]>,
        client_certificate: Option<&[u8]>,
        client_private_key: Option<&[u8]>,
    ) -> Result<Self, IdentityError> {
        if redis_url.trim().is_empty()
            || namespace.trim().is_empty()
            || namespace.len() > 128
            || !namespace
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'_' | b'-'))
        {
            return Err(IdentityError::PersistenceUnavailable);
        }
        let redis_endpoint =
            url::Url::parse(redis_url).map_err(|_| IdentityError::PersistenceUnavailable)?;
        let host = redis_endpoint
            .host_str()
            .ok_or(IdentityError::PersistenceUnavailable)?;
        let local = matches!(host, "localhost" | "127.0.0.1" | "::1");
        if redis_endpoint.scheme() != "rediss" && !(local && redis_endpoint.scheme() == "redis") {
            return Err(IdentityError::PersistenceUnavailable);
        }
        let material = [
            root_certificate.is_some(),
            client_certificate.is_some(),
            client_private_key.is_some(),
        ];
        if material.iter().any(|present| *present) && !material.iter().all(|present| *present) {
            return Err(IdentityError::PersistenceUnavailable);
        }
        let client = if redis_endpoint.scheme() == "rediss" {
            redis::Client::build_with_tls(
                redis_url,
                redis::TlsCertificates {
                    client_tls: Some(redis::ClientTlsConfig {
                        client_cert: client_certificate
                            .ok_or(IdentityError::PersistenceUnavailable)?
                            .to_vec(),
                        client_key: client_private_key
                            .ok_or(IdentityError::PersistenceUnavailable)?
                            .to_vec(),
                    }),
                    root_cert: Some(
                        root_certificate
                            .ok_or(IdentityError::PersistenceUnavailable)?
                            .to_vec(),
                    ),
                },
            )
            .map_err(|_| IdentityError::PersistenceUnavailable)?
        } else {
            redis::Client::open(redis_url).map_err(|_| IdentityError::PersistenceUnavailable)?
        };
        let security = Self {
            client,
            namespace: namespace.to_owned(),
        };
        security.check_readiness()?;
        Ok(security)
    }

    fn key(&self, login_key: &str) -> String {
        format!("{}:login:{}", self.namespace, login_key)
    }

    fn reserve(&self, login_key: &str) -> Result<(), IdentityError> {
        let mut connection = self
            .client
            .get_connection()
            .map_err(|_| IdentityError::PersistenceUnavailable)?;
        let allowed: i64 = redis::Script::new(DISTRIBUTED_LOGIN_SCRIPT)
            .key(self.key(login_key))
            .arg(LOGIN_FAILURE_LIMIT)
            .arg(LOGIN_FAILURE_WINDOW_MS)
            .arg(LOGIN_BLOCK_MS)
            .invoke(&mut connection)
            .map_err(|_| IdentityError::PersistenceUnavailable)?;
        if allowed == 1 {
            Ok(())
        } else {
            Err(IdentityError::LoginRateLimited)
        }
    }

    fn clear(&self, login_key: &str) -> Result<(), IdentityError> {
        let mut connection = self
            .client
            .get_connection()
            .map_err(|_| IdentityError::PersistenceUnavailable)?;
        redis::cmd("DEL")
            .arg(self.key(login_key))
            .query::<i64>(&mut connection)
            .map(|_| ())
            .map_err(|_| IdentityError::PersistenceUnavailable)
    }

    fn check_readiness(&self) -> Result<(), IdentityError> {
        let mut connection = self
            .client
            .get_connection()
            .map_err(|_| IdentityError::PersistenceUnavailable)?;
        let response: String = redis::cmd("PING")
            .query(&mut connection)
            .map_err(|_| IdentityError::PersistenceUnavailable)?;
        if response == "PONG" {
            Ok(())
        } else {
            Err(IdentityError::PersistenceUnavailable)
        }
    }
}

#[derive(Debug)]
struct PasswordVerificationState {
    active: usize,
}

#[derive(Debug)]
struct PasswordVerificationGate {
    limit: usize,
    state: Mutex<PasswordVerificationState>,
}

impl PasswordVerificationGate {
    fn new(limit: usize) -> Result<Self, IdentityError> {
        if limit == 0 || limit > 64 {
            return Err(IdentityError::InvalidIdentityData);
        }
        Ok(Self {
            limit,
            state: Mutex::new(PasswordVerificationState { active: 0 }),
        })
    }

    fn try_acquire(&self) -> Result<PasswordVerificationPermit<'_>, IdentityError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| IdentityError::PersistenceUnavailable)?;
        if state.active >= self.limit {
            return Err(IdentityError::LoginRateLimited);
        }
        state.active += 1;
        Ok(PasswordVerificationPermit { gate: self })
    }
}

struct PasswordVerificationPermit<'a> {
    gate: &'a PasswordVerificationGate,
}

impl Drop for PasswordVerificationPermit<'_> {
    fn drop(&mut self) {
        if let Ok(mut state) = self.gate.state.lock() {
            state.active = state.active.saturating_sub(1);
        }
    }
}
