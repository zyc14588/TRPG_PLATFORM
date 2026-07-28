
impl AuthenticationContext {
    pub fn subject_id(&self) -> &EntityId {
        &self.subject_id
    }

    pub fn kind(&self) -> &PrincipalKind {
        &self.kind
    }

    pub const fn authenticated_at_unix_ms(&self) -> u64 {
        self.authenticated_at_unix_ms
    }

    pub const fn expires_at_unix_ms(&self) -> u64 {
        self.expires_at_unix_ms
    }

    pub fn require_campaign(&self, campaign_id: &EntityId) -> Result<(), IdentityError> {
        if let PrincipalKind::AgentRun {
            campaign_id: bound_campaign,
            ..
        } = &self.kind
        {
            if bound_campaign != campaign_id {
                return Err(IdentityError::CampaignScopeMismatch);
            }
        }
        Ok(())
    }

    fn to_command_actor(
        &self,
        membership: Option<&CampaignMembership>,
    ) -> Result<Actor, IdentityError> {
        match &self.kind {
            PrincipalKind::UserSession {
                session_id,
                global_role,
            } => {
                if membership.is_some_and(|membership| membership.user_id != self.subject_id) {
                    return Err(IdentityError::MembershipDenied);
                }
                let role = match (global_role, membership.map(CampaignMembership::role)) {
                    (GlobalRole::ServerOwner, _) => ActorRole::ServerOwner,
                    (GlobalRole::Moderator, _) => ActorRole::Moderator,
                    (_, Some(CampaignRole::CampaignOwner)) => ActorRole::CampaignOwner,
                    (_, Some(CampaignRole::HumanKeeper)) => ActorRole::HumanKeeper,
                    (_, Some(CampaignRole::Player)) => ActorRole::Investigator,
                    (_, Some(CampaignRole::Spectator)) => ActorRole::Spectator,
                    (_, None) => return Err(IdentityError::MembershipRequired),
                };
                Actor::authenticated_user(self.subject_id.as_str(), role, session_id.as_str())
            }
            PrincipalKind::Workload { role } => Actor::verified_workload(
                self.subject_id.as_str(),
                match role {
                    WorkloadRole::ApiServer => KernelWorkloadRole::ApiServer,
                    WorkloadRole::RealtimeServer => KernelWorkloadRole::RealtimeServer,
                    WorkloadRole::AgentWorker => KernelWorkloadRole::AgentWorker,
                    WorkloadRole::WorkflowEngine => KernelWorkloadRole::WorkflowEngine,
                    WorkloadRole::RulesEngine => KernelWorkloadRole::RulesEngine,
                    WorkloadRole::AuditWriter => KernelWorkloadRole::AuditWriter,
                },
            ),
            PrincipalKind::AgentRun {
                run_id,
                class,
                campaign_id,
            } => Actor::verified_agent_run(
                self.subject_id.as_str(),
                run_id.as_str(),
                match class {
                    AgentClass::AiKeeperOrchestrator => KernelAgentClass::AiKeeperOrchestrator,
                    AgentClass::KeeperCopilot => KernelAgentClass::KeeperCopilot,
                    AgentClass::AtmosphereWriter => KernelAgentClass::AtmosphereWriter,
                    AgentClass::MemoryCurator => KernelAgentClass::MemoryCurator,
                },
                campaign_id.as_str(),
            ),
        }
        .map_err(|_| IdentityError::InvalidInternalCredential)
    }
}

/// Read-only trust anchor distributed to services that consume authenticated
/// identities. The fingerprint is not a signing secret; contexts remain
/// constructible only by `IdentityService` and are accepted only from the
/// configured issuer.
#[derive(Clone)]
pub struct IdentityVerifier {
    issuer_fingerprint: [u8; 32],
    state: Arc<RwLock<VerificationState>>,
    persistent_verification: Option<PersistentVerificationStore>,
}

impl fmt::Debug for IdentityVerifier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IdentityVerifier")
            .field("issuer_fingerprint", &hex_encode(&self.issuer_fingerprint))
            .field("state", &"[LIVE IDENTITY STATE]")
            .field(
                "persistent_verification",
                &self
                    .persistent_verification
                    .as_ref()
                    .map(|_| "[POSTGRESQL]"),
            )
            .finish()
    }
}

impl IdentityVerifier {
    pub fn verify(
        &self,
        authentication: &AuthenticationContext,
        now_unix_ms: u64,
    ) -> Result<(), IdentityError> {
        if authentication.issuer_fingerprint != self.issuer_fingerprint {
            return Err(IdentityError::InvalidInternalCredential);
        }
        if authentication.authenticated_at_unix_ms > now_unix_ms
            || authentication.expires_at_unix_ms <= now_unix_ms
        {
            return Err(IdentityError::InternalCredentialExpired);
        }
        if let PrincipalKind::UserSession { session_id, .. } = authentication.kind() {
            if let Some(persistent) = &self.persistent_verification {
                return persistent.verify_session(
                    session_id,
                    &authentication.subject_id,
                    authentication.authenticated_at_unix_ms,
                    authentication.expires_at_unix_ms,
                );
            }
            let state = self
                .state
                .read()
                .map_err(|_| IdentityError::PersistenceUnavailable)?;
            let session = state
                .sessions_by_id
                .get(session_id)
                .ok_or(IdentityError::SessionNotFound)?;
            if session.user_id != authentication.subject_id
                || session.issued_at_unix_ms != authentication.authenticated_at_unix_ms
                || session.expires_at_unix_ms != authentication.expires_at_unix_ms
            {
                return Err(IdentityError::InvalidInternalCredential);
            }
            if session.revoked {
                return Err(IdentityError::SessionRevoked);
            }
        }
        Ok(())
    }

    pub fn authority_contract(
        &self,
        campaign_id: &EntityId,
    ) -> Result<AuthorityContract, IdentityError> {
        self.state
            .read()
            .map_err(|_| IdentityError::PersistenceUnavailable)?
            .authorities
            .get(campaign_id)
            .cloned()
            .ok_or(IdentityError::AuthorityContractRequired)
    }

    pub fn authorize_replay(
        &self,
        authentication: &AuthenticationContext,
        campaign_id: &EntityId,
        now_unix_ms: u64,
    ) -> Result<ReplayAuthorization, IdentityError> {
        self.verify(authentication, now_unix_ms)?;
        authentication.require_campaign(campaign_id)?;
        let binding = match authentication.kind() {
            PrincipalKind::UserSession { session_id, .. } => {
                if let Some(persistent) = &self.persistent_verification {
                    persistent.replay_principal(
                        session_id,
                        authentication.subject_id(),
                        authentication.authenticated_at_unix_ms,
                        authentication.expires_at_unix_ms,
                        campaign_id,
                    )?;
                } else {
                    let state = self
                        .state
                        .read()
                        .map_err(|_| IdentityError::PersistenceUnavailable)?;
                    if !state
                        .memberships
                        .contains_key(&(campaign_id.clone(), authentication.subject_id.clone()))
                    {
                        return Err(IdentityError::MembershipRequired);
                    }
                }
                ReplayBinding::UserSession {
                    session_id: session_id.clone(),
                }
            }
            PrincipalKind::Workload { .. } => ReplayBinding::Workload,
            PrincipalKind::AgentRun { .. } => return Err(IdentityError::MembershipDenied),
        };
        Ok(ReplayAuthorization {
            subject_id: authentication.subject_id.clone(),
            binding,
            campaign_id: campaign_id.clone(),
            authenticated_at_unix_ms: authentication.authenticated_at_unix_ms,
            expires_at_unix_ms: authentication.expires_at_unix_ms,
            state: Arc::clone(&self.state),
            persistent_verification: self.persistent_verification.clone(),
        })
    }

    pub fn verify_actor(
        &self,
        authentication: &AuthenticationContext,
        actor: &Actor,
        campaign_id: &EntityId,
        now_unix_ms: u64,
    ) -> Result<(), IdentityError> {
        self.verify(authentication, now_unix_ms)?;
        authentication.require_campaign(campaign_id)?;
        if actor.id() != authentication.subject_id() {
            return Err(IdentityError::InvalidInternalCredential);
        }
        match (authentication.kind(), actor.origin()) {
            (
                PrincipalKind::UserSession {
                    session_id,
                    global_role,
                },
                ActorOrigin::UserSession {
                    session_id: actor_session_id,
                },
            ) if session_id == actor_session_id => {
                let expected_role = match global_role {
                    GlobalRole::ServerOwner => ActorRole::ServerOwner,
                    GlobalRole::Moderator => ActorRole::Moderator,
                    GlobalRole::User => {
                        let role = if let Some(persistent) = &self.persistent_verification {
                            persistent.campaign_role(campaign_id, authentication.subject_id())?
                        } else {
                            *self
                                .state
                                .read()
                                .map_err(|_| IdentityError::PersistenceUnavailable)?
                                .memberships
                                .get(&(campaign_id.clone(), authentication.subject_id().clone()))
                                .ok_or(IdentityError::MembershipRequired)?
                        };
                        match role {
                            CampaignRole::CampaignOwner => ActorRole::CampaignOwner,
                            CampaignRole::HumanKeeper => ActorRole::HumanKeeper,
                            CampaignRole::Player => ActorRole::Investigator,
                            CampaignRole::Spectator => ActorRole::Spectator,
                        }
                    }
                };
                if actor.role() != &expected_role {
                    return Err(IdentityError::InvalidInternalCredential);
                }
            }
            (PrincipalKind::Workload { role }, ActorOrigin::Workload { role: actor_role }) => {
                let (expected_origin, expected_role) = match role {
                    WorkloadRole::ApiServer => (KernelWorkloadRole::ApiServer, ActorRole::System),
                    WorkloadRole::RealtimeServer => {
                        (KernelWorkloadRole::RealtimeServer, ActorRole::System)
                    }
                    WorkloadRole::AgentWorker => {
                        (KernelWorkloadRole::AgentWorker, ActorRole::System)
                    }
                    WorkloadRole::WorkflowEngine => {
                        (KernelWorkloadRole::WorkflowEngine, ActorRole::Workflow)
                    }
                    WorkloadRole::RulesEngine => {
                        (KernelWorkloadRole::RulesEngine, ActorRole::RulesEngine)
                    }
                    WorkloadRole::AuditWriter => {
                        (KernelWorkloadRole::AuditWriter, ActorRole::System)
                    }
                };
                if actor_role != &expected_origin || actor.role() != &expected_role {
                    return Err(IdentityError::InvalidInternalCredential);
                }
            }
            (
                PrincipalKind::AgentRun {
                    run_id,
                    class,
                    campaign_id: bound_campaign,
                },
                ActorOrigin::AgentRun {
                    run_id: actor_run_id,
                    class: actor_class,
                    campaign_id: actor_campaign,
                },
            ) => {
                let (expected_class, expected_role) = match class {
                    AgentClass::AiKeeperOrchestrator => {
                        (KernelAgentClass::AiKeeperOrchestrator, ActorRole::AiKeeper)
                    }
                    AgentClass::KeeperCopilot => {
                        (KernelAgentClass::KeeperCopilot, ActorRole::Investigator)
                    }
                    AgentClass::AtmosphereWriter => {
                        (KernelAgentClass::AtmosphereWriter, ActorRole::Investigator)
                    }
                    AgentClass::MemoryCurator => {
                        (KernelAgentClass::MemoryCurator, ActorRole::Investigator)
                    }
                };
                if run_id != actor_run_id
                    || bound_campaign != campaign_id
                    || actor_campaign != campaign_id
                    || actor_class != &expected_class
                    || actor.role() != &expected_role
                {
                    return Err(IdentityError::InvalidInternalCredential);
                }
            }
            _ => return Err(IdentityError::InvalidInternalCredential),
        }
        Ok(())
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct SessionToken(String);

impl SessionToken {
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SessionToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SessionToken([REDACTED])")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoginSession {
    pub token: SessionToken,
    pub expires_at_unix_ms: u64,
}

#[derive(Clone, Debug)]
struct UserRecord {
    user_id: EntityId,
    password_hash: String,
    global_role: GlobalRole,
}
