
impl PersistentVerificationStore {
    fn new(database: Client) -> Self {
        Self {
            database: Arc::new(Mutex::new(database)),
        }
    }

    fn check_readiness(&self) -> Result<(), IdentityError> {
        self.database
            .lock()
            .map_err(|_| IdentityError::PersistenceUnavailable)?
            .check_connection()
            .map_err(|_| IdentityError::PersistenceUnavailable)
    }

    fn verify_session(
        &self,
        session_id: &EntityId,
        subject_id: &EntityId,
        issued_at_unix_ms: u64,
        expires_at_unix_ms: u64,
    ) -> Result<(), IdentityError> {
        let mut database = self
            .database
            .lock()
            .map_err(|_| IdentityError::PersistenceUnavailable)?;
        verify_persisted_session(
            &mut database,
            session_id,
            subject_id,
            issued_at_unix_ms,
            expires_at_unix_ms,
        )
    }

    fn replay_principal(
        &self,
        session_id: &EntityId,
        subject_id: &EntityId,
        issued_at_unix_ms: u64,
        expires_at_unix_ms: u64,
        campaign_id: &EntityId,
    ) -> Result<PrincipalScope, IdentityError> {
        let mut database = self
            .database
            .lock()
            .map_err(|_| IdentityError::PersistenceUnavailable)?;
        verify_persisted_session(
            &mut database,
            session_id,
            subject_id,
            issued_at_unix_ms,
            expires_at_unix_ms,
        )?;
        let row = database
            .query_opt(
                "SELECT role FROM campaign_memberships \
                   WHERE campaign_id = $1 AND user_id = $2 AND revoked_at IS NULL",
                &[&campaign_id.as_str(), &subject_id.as_str()],
            )
            .map_err(|_| IdentityError::PersistenceUnavailable)?
            .ok_or(IdentityError::MembershipRequired)?;
        Ok(
            match parse_campaign_role(row.get::<_, String>(0).as_str())? {
                CampaignRole::HumanKeeper => PrincipalScope::Keeper,
                CampaignRole::Player => PrincipalScope::Player(subject_id.clone()),
                CampaignRole::CampaignOwner => PrincipalScope::PartyMember,
                CampaignRole::Spectator => PrincipalScope::Spectator,
            },
        )
    }

    /// Rechecks the session, campaign role, and optional private-group grant
    /// in one PostgreSQL statement. PostgreSQL gives one statement a single
    /// MVCC snapshot, so a membership revocation cannot be interleaved
    /// between a role query and a separate group query.
    fn can_view(
        &self,
        session_id: &EntityId,
        subject_id: &EntityId,
        issued_at_unix_ms: u64,
        expires_at_unix_ms: u64,
        campaign_id: &EntityId,
        visibility: &Visibility,
    ) -> Result<bool, IdentityError> {
        let group_id = visibility.group_id().map(EntityId::as_str);
        let row = self
            .database
            .lock()
            .map_err(|_| IdentityError::PersistenceUnavailable)?
            .query_opt(
                "SELECT session.user_id, \
                        (extract(epoch FROM session.issued_at) * 1000)::bigint, \
                        (extract(epoch FROM session.expires_at) * 1000)::bigint, \
                        session.revoked_at IS NOT NULL, membership.role, \
                        CASE WHEN $4::text IS NULL THEN false ELSE EXISTS (\
                            SELECT 1 FROM campaign_group_memberships AS group_membership \
                             WHERE group_membership.campaign_id = $3 \
                               AND group_membership.group_id = $4 \
                               AND group_membership.user_id = $2 \
                               AND group_membership.revoked_at IS NULL\
                        ) END \
                   FROM sessions AS session \
                   LEFT JOIN campaign_memberships AS membership \
                     ON membership.campaign_id = $3 \
                    AND membership.user_id = session.user_id \
                    AND membership.revoked_at IS NULL \
                  WHERE session.session_id = $1 \
                    AND session.user_id = $2",
                &[
                    &session_id.as_str(),
                    &subject_id.as_str(),
                    &campaign_id.as_str(),
                    &group_id,
                ],
            )
            .map_err(|_| IdentityError::PersistenceUnavailable)?
            .ok_or(IdentityError::SessionNotFound)?;
        if row.get::<_, String>(0) != subject_id.as_str()
            || row.get::<_, i64>(1)
                != i64::try_from(issued_at_unix_ms)
                    .map_err(|_| IdentityError::InvalidInternalCredential)?
            || row.get::<_, i64>(2)
                != i64::try_from(expires_at_unix_ms)
                    .map_err(|_| IdentityError::InvalidInternalCredential)?
        {
            return Err(IdentityError::InvalidInternalCredential);
        }
        if row.get::<_, bool>(3) {
            return Err(IdentityError::SessionRevoked);
        }
        let role = row
            .get::<_, Option<String>>(4)
            .ok_or(IdentityError::MembershipRequired)?;
        let principal = match parse_campaign_role(&role)? {
            CampaignRole::HumanKeeper => PrincipalScope::Keeper,
            CampaignRole::Player => PrincipalScope::Player(subject_id.clone()),
            CampaignRole::CampaignOwner => PrincipalScope::PartyMember,
            CampaignRole::Spectator => PrincipalScope::Spectator,
        };
        if visibility.label().kind() == VisibilityKind::PrivateToGroup {
            return Ok(match principal {
                PrincipalScope::Keeper | PrincipalScope::System => true,
                PrincipalScope::Player(_) | PrincipalScope::PartyMember => {
                    group_id.is_some() && row.get::<_, bool>(5)
                }
                _ => false,
            });
        }
        Ok(visibility.can_view(&principal))
    }

    fn campaign_role(
        &self,
        campaign_id: &EntityId,
        subject_id: &EntityId,
    ) -> Result<CampaignRole, IdentityError> {
        let row = self
            .database
            .lock()
            .map_err(|_| IdentityError::PersistenceUnavailable)?
            .query_opt(
                "SELECT role FROM campaign_memberships \
                   WHERE campaign_id = $1 AND user_id = $2 AND revoked_at IS NULL",
                &[&campaign_id.as_str(), &subject_id.as_str()],
            )
            .map_err(|_| IdentityError::PersistenceUnavailable)?
            .ok_or(IdentityError::MembershipRequired)?;
        parse_campaign_role(row.get::<_, String>(0).as_str())
    }
}

fn verify_persisted_session(
    database: &mut Client,
    session_id: &EntityId,
    subject_id: &EntityId,
    issued_at_unix_ms: u64,
    expires_at_unix_ms: u64,
) -> Result<(), IdentityError> {
    let row = database
        .query_opt(
            "SELECT user_id, \
                    (extract(epoch FROM issued_at) * 1000)::bigint, \
                    (extract(epoch FROM expires_at) * 1000)::bigint, \
                    revoked_at IS NOT NULL \
               FROM sessions WHERE session_id = $1",
            &[&session_id.as_str()],
        )
        .map_err(|_| IdentityError::PersistenceUnavailable)?
        .ok_or(IdentityError::SessionNotFound)?;
    if row.get::<_, String>(0) != subject_id.as_str()
        || row.get::<_, i64>(1) != i64::try_from(issued_at_unix_ms).unwrap_or(-1)
        || row.get::<_, i64>(2) != i64::try_from(expires_at_unix_ms).unwrap_or(-1)
    {
        return Err(IdentityError::InvalidInternalCredential);
    }
    if row.get::<_, bool>(3) {
        return Err(IdentityError::SessionRevoked);
    }
    Ok(())
}

/// Opaque, live replay capability minted from an authenticated user session.
///
/// The capability is campaign-bound and rechecks session revocation and the
/// current campaign membership on every visibility decision. Callers cannot
/// manufacture Keeper or System replay authority from a public enum value.
#[derive(Clone)]
pub struct ReplayAuthorization {
    subject_id: EntityId,
    binding: ReplayBinding,
    campaign_id: EntityId,
    authenticated_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    state: Arc<RwLock<VerificationState>>,
    persistent_verification: Option<PersistentVerificationStore>,
}

#[derive(Clone, Debug)]
enum ReplayBinding {
    UserSession { session_id: EntityId },
    Workload,
}

impl fmt::Debug for ReplayAuthorization {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReplayAuthorization")
            .field("subject_id", &self.subject_id)
            .field("campaign_id", &self.campaign_id)
            .field("expires_at_unix_ms", &self.expires_at_unix_ms)
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

impl ReplayAuthorization {
    pub fn campaign_id(&self) -> &EntityId {
        &self.campaign_id
    }

    pub fn subject_id(&self) -> &EntityId {
        &self.subject_id
    }

    pub fn can_view(
        &self,
        event_campaign_id: &EntityId,
        visibility: &Visibility,
        now_unix_ms: u64,
    ) -> Result<bool, IdentityError> {
        if event_campaign_id != &self.campaign_id {
            return Ok(false);
        }
        if now_unix_ms < self.authenticated_at_unix_ms || now_unix_ms >= self.expires_at_unix_ms {
            return Err(IdentityError::SessionExpired);
        }
        let principal = match &self.binding {
            ReplayBinding::Workload => PrincipalScope::System,
            ReplayBinding::UserSession { session_id } => {
                if let Some(persistent) = &self.persistent_verification {
                    return persistent.can_view(
                        session_id,
                        &self.subject_id,
                        self.authenticated_at_unix_ms,
                        self.expires_at_unix_ms,
                        &self.campaign_id,
                        visibility,
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
                if session.revoked {
                    return Err(IdentityError::SessionRevoked);
                }
                if session.user_id != self.subject_id
                    || session.issued_at_unix_ms != self.authenticated_at_unix_ms
                    || session.expires_at_unix_ms != self.expires_at_unix_ms
                {
                    return Err(IdentityError::InvalidInternalCredential);
                }
                match state
                    .memberships
                    .get(&(self.campaign_id.clone(), self.subject_id.clone()))
                    .ok_or(IdentityError::MembershipRequired)?
                {
                    CampaignRole::HumanKeeper => PrincipalScope::Keeper,
                    // Campaign role is not proof of membership in an arbitrary
                    // private group. Ordinary players remain Player principals;
                    // the private-group branch below separately verifies the
                    // authoritative live campaign/group/subject tuple.
                    CampaignRole::Player => PrincipalScope::Player(self.subject_id.clone()),
                    CampaignRole::CampaignOwner => PrincipalScope::PartyMember,
                    CampaignRole::Spectator => PrincipalScope::Spectator,
                }
            }
        };
        if visibility.label().kind() == VisibilityKind::PrivateToGroup {
            let state = self
                .state
                .read()
                .map_err(|_| IdentityError::PersistenceUnavailable)?;
            return Ok(match principal {
                PrincipalScope::Keeper | PrincipalScope::System => true,
                PrincipalScope::Player(_) | PrincipalScope::PartyMember => {
                    visibility.group_id().is_some_and(|group_id| {
                        state.group_memberships.contains(&(
                            self.campaign_id.clone(),
                            group_id.clone(),
                            self.subject_id.clone(),
                        ))
                    })
                }
                PrincipalScope::Public
                | PrincipalScope::GroupMember(_)
                | PrincipalScope::Spectator => false,
                PrincipalScope::Claims(_) => visibility.can_view(&principal),
            });
        }
        Ok(visibility.can_view(&principal))
    }
}
