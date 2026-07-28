
impl IdentityService {

    pub fn authenticate_session(
        &mut self,
        token: Option<&str>,
        now_unix_ms: u64,
    ) -> Result<AuthenticationContext, IdentityError> {
        self.sync_if_persistent()?;
        let token = token.ok_or(IdentityError::AuthenticationRequired)?;
        let token_hash = hash_token(token);
        let session = self
            .sessions_by_hash
            .get(&token_hash)
            .ok_or(IdentityError::SessionNotFound)?;
        if session.revoked {
            return Err(IdentityError::SessionRevoked);
        }
        if now_unix_ms >= session.expires_at_unix_ms {
            return Err(IdentityError::SessionExpired);
        }
        let user = self
            .users_by_id
            .get(&session.user_id)
            .ok_or(IdentityError::InvalidIdentityData)?;
        Ok(AuthenticationContext {
            subject_id: user.user_id.clone(),
            kind: PrincipalKind::UserSession {
                session_id: session.session_id.clone(),
                global_role: user.global_role,
            },
            authenticated_at_unix_ms: session.issued_at_unix_ms,
            expires_at_unix_ms: session.expires_at_unix_ms,
            issuer_fingerprint: self.verifier().issuer_fingerprint,
        })
    }

    pub fn refresh_session(
        &mut self,
        token: &str,
        now_unix_ms: u64,
    ) -> Result<LoginSession, IdentityError> {
        self.sync_if_persistent()?;
        let token_hash = hash_token(token);
        let session = self
            .sessions_by_hash
            .get(&token_hash)
            .ok_or(IdentityError::SessionNotFound)?;
        if session.revoked {
            return Err(IdentityError::SessionRevoked);
        }
        if now_unix_ms >= session.expires_at_unix_ms {
            return Err(IdentityError::SessionExpired);
        }
        let user_id = session.user_id.clone();
        let rotated_from_session_id = session.session_id.clone();
        let (replacement_hash, replacement_record, replacement) =
            self.generate_session(user_id, now_unix_ms)?;
        if let Some(database) = self.database.as_mut() {
            let mut transaction = database
                .transaction()
                .map_err(|_| IdentityError::PersistenceUnavailable)?;
            let updated = transaction
                .execute(
                    "UPDATE sessions SET revoked_at = now() \
                     WHERE token_hash = $1 AND revoked_at IS NULL \
                       AND expires_at > to_timestamp($2::bigint / 1000.0)",
                    &[
                        &&token_hash[..],
                        &i64::try_from(now_unix_ms)
                            .map_err(|_| IdentityError::InvalidIdentityData)?,
                    ],
                )
                .map_err(|_| IdentityError::PersistenceUnavailable)?;
            if updated != 1 {
                return Err(IdentityError::SessionRevoked);
            }
            persist_session(
                &mut transaction,
                replacement_hash,
                &replacement_record,
                Some(&rotated_from_session_id),
            )?;
            transaction
                .commit()
                .map_err(|_| IdentityError::PersistenceUnavailable)?;
        }
        self.sessions_by_hash
            .get_mut(&token_hash)
            .ok_or(IdentityError::SessionNotFound)?
            .revoked = true;
        self.sessions_by_hash
            .insert(replacement_hash, replacement_record);
        self.publish_verification_state()?;
        Ok(replacement)
    }

    pub fn logout(&mut self, token: &str) -> Result<(), IdentityError> {
        self.sync_if_persistent()?;
        let token_hash = hash_token(token);
        let session = self
            .sessions_by_hash
            .get(&token_hash)
            .ok_or(IdentityError::SessionNotFound)?;
        if session.revoked {
            return Err(IdentityError::SessionRevoked);
        }
        if let Some(database) = self.database.as_mut() {
            let updated = database
                .execute(
                    "UPDATE sessions SET revoked_at = now() \
                     WHERE token_hash = $1 AND revoked_at IS NULL",
                    &[&&token_hash[..]],
                )
                .map_err(|_| IdentityError::PersistenceUnavailable)?;
            if updated != 1 {
                return Err(IdentityError::SessionRevoked);
            }
        }
        self.sessions_by_hash
            .get_mut(&token_hash)
            .ok_or(IdentityError::SessionNotFound)?
            .revoked = true;
        self.publish_verification_state()
    }

    pub fn grant_membership(
        &mut self,
        actor: &AuthenticationContext,
        campaign_id: impl Into<String>,
        user_id: impl Into<String>,
        role: CampaignRole,
        now_unix_ms: u64,
    ) -> Result<CampaignMembership, IdentityError> {
        self.sync_if_persistent()?;
        self.verifier().verify(actor, now_unix_ms)?;
        let campaign_id =
            EntityId::new(campaign_id).map_err(|_| IdentityError::InvalidIdentityData)?;
        let user_id = EntityId::new(user_id).map_err(|_| IdentityError::InvalidIdentityData)?;
        if !self.users_by_id.contains_key(&user_id) {
            return Err(IdentityError::InvalidIdentityData);
        }
        let server_owner = matches!(
            actor.kind,
            PrincipalKind::UserSession {
                global_role: GlobalRole::ServerOwner,
                ..
            }
        );
        if !self.can_manage_campaign_memberships(actor, &campaign_id) {
            return Err(IdentityError::MembershipDenied);
        }
        if !server_owner
            && matches!(
                role,
                CampaignRole::CampaignOwner | CampaignRole::HumanKeeper
            )
        {
            return Err(IdentityError::MembershipDenied);
        }
        if role == CampaignRole::HumanKeeper
            && self.memberships.values().any(|membership| {
                membership.campaign_id == campaign_id
                    && membership.user_id != user_id
                    && membership.role == CampaignRole::HumanKeeper
            })
        {
            return Err(IdentityError::MembershipDenied);
        }
        if let Some(contract) = self.authorities.get(&campaign_id) {
            if (role == CampaignRole::HumanKeeper
                && (contract.mode() != &AuthorityMode::HumanKp
                    || contract.authority_owner() != &user_id))
                || (contract.mode() == &AuthorityMode::HumanKp
                    && contract.authority_owner() == &user_id
                    && role != CampaignRole::HumanKeeper)
            {
                return Err(IdentityError::MembershipDenied);
            }
        }
        let membership = CampaignMembership {
            campaign_id: campaign_id.clone(),
            user_id: user_id.clone(),
            role,
        };
        if let Some(database) = self.database.as_mut() {
            database
                .execute(
                    "INSERT INTO campaign_memberships \
                        (campaign_id, user_id, role, granted_by, revoked_at) \
                     VALUES ($1, $2, $3, $4, NULL) \
                     ON CONFLICT (campaign_id, user_id) DO UPDATE SET \
                        role = EXCLUDED.role, granted_by = EXCLUDED.granted_by, \
                        granted_at = now(), revoked_at = NULL",
                    &[
                        &membership.campaign_id.as_str(),
                        &membership.user_id.as_str(),
                        &campaign_role_name(membership.role),
                        &actor.subject_id.as_str(),
                    ],
                )
                .map_err(map_postgres_error)?;
        }
        self.memberships
            .insert((campaign_id, user_id), membership.clone());
        self.publish_verification_state()?;
        Ok(membership)
    }

    pub fn create_campaign_group(
        &mut self,
        actor: &AuthenticationContext,
        campaign_id: impl Into<String>,
        group_id: impl Into<String>,
        now_unix_ms: u64,
    ) -> Result<CampaignGroup, IdentityError> {
        self.sync_if_persistent()?;
        self.verifier().verify(actor, now_unix_ms)?;
        let campaign_id =
            EntityId::new(campaign_id).map_err(|_| IdentityError::InvalidIdentityData)?;
        let group_id = EntityId::new(group_id).map_err(|_| IdentityError::InvalidIdentityData)?;
        if !self.can_manage_campaign_memberships(actor, &campaign_id) {
            return Err(IdentityError::MembershipDenied);
        }
        if !self
            .memberships
            .keys()
            .any(|(member_campaign_id, _)| member_campaign_id == &campaign_id)
        {
            return Err(IdentityError::MembershipRequired);
        }
        let key = (campaign_id.clone(), group_id.clone());
        if let Some(existing) = self.campaign_groups.get(&key) {
            return Ok(existing.clone());
        }
        let group = CampaignGroup {
            campaign_id,
            group_id,
        };
        if let Some(database) = self.database.as_mut() {
            database
                .execute(
                    "INSERT INTO campaign_groups (campaign_id, group_id, created_by) \
                     VALUES ($1, $2, $3)",
                    &[
                        &group.campaign_id.as_str(),
                        &group.group_id.as_str(),
                        &actor.subject_id.as_str(),
                    ],
                )
                .map_err(map_postgres_error)?;
        }
        self.campaign_groups.insert(key, group.clone());
        Ok(group)
    }

    pub fn grant_group_membership(
        &mut self,
        actor: &AuthenticationContext,
        campaign_id: impl Into<String>,
        group_id: impl Into<String>,
        user_id: impl Into<String>,
        now_unix_ms: u64,
    ) -> Result<CampaignGroupMembership, IdentityError> {
        self.sync_if_persistent()?;
        self.verifier().verify(actor, now_unix_ms)?;
        let campaign_id =
            EntityId::new(campaign_id).map_err(|_| IdentityError::InvalidIdentityData)?;
        let group_id = EntityId::new(group_id).map_err(|_| IdentityError::InvalidIdentityData)?;
        let user_id = EntityId::new(user_id).map_err(|_| IdentityError::InvalidIdentityData)?;
        if !self.can_manage_campaign_memberships(actor, &campaign_id) {
            return Err(IdentityError::MembershipDenied);
        }
        if !self
            .campaign_groups
            .contains_key(&(campaign_id.clone(), group_id.clone()))
        {
            return Err(IdentityError::MembershipRequired);
        }
        let campaign_membership = self
            .memberships
            .get(&(campaign_id.clone(), user_id.clone()))
            .ok_or(IdentityError::MembershipRequired)?;
        if !matches!(
            campaign_membership.role,
            CampaignRole::CampaignOwner | CampaignRole::Player
        ) {
            return Err(IdentityError::MembershipDenied);
        }
        let membership = CampaignGroupMembership {
            campaign_id: campaign_id.clone(),
            group_id: group_id.clone(),
            user_id: user_id.clone(),
        };
        if let Some(database) = self.database.as_mut() {
            database
                .execute(
                    "INSERT INTO campaign_group_memberships \
                        (campaign_id, group_id, user_id, granted_by, revoked_at) \
                     VALUES ($1, $2, $3, $4, NULL) \
                     ON CONFLICT (campaign_id, group_id, user_id) DO UPDATE SET \
                        granted_by = EXCLUDED.granted_by, granted_at = now(), revoked_at = NULL",
                    &[
                        &membership.campaign_id.as_str(),
                        &membership.group_id.as_str(),
                        &membership.user_id.as_str(),
                        &actor.subject_id.as_str(),
                    ],
                )
                .map_err(map_postgres_error)?;
        }
        self.group_memberships
            .insert((campaign_id, group_id, user_id), membership.clone());
        self.publish_verification_state()?;
        Ok(membership)
    }

    pub fn revoke_group_membership(
        &mut self,
        actor: &AuthenticationContext,
        campaign_id: impl Into<String>,
        group_id: impl Into<String>,
        user_id: impl Into<String>,
        now_unix_ms: u64,
    ) -> Result<(), IdentityError> {
        self.sync_if_persistent()?;
        self.verifier().verify(actor, now_unix_ms)?;
        let campaign_id =
            EntityId::new(campaign_id).map_err(|_| IdentityError::InvalidIdentityData)?;
        let group_id = EntityId::new(group_id).map_err(|_| IdentityError::InvalidIdentityData)?;
        let user_id = EntityId::new(user_id).map_err(|_| IdentityError::InvalidIdentityData)?;
        if !self.can_manage_campaign_memberships(actor, &campaign_id) {
            return Err(IdentityError::MembershipDenied);
        }
        let key = (campaign_id.clone(), group_id.clone(), user_id.clone());
        if !self.group_memberships.contains_key(&key) {
            return Err(IdentityError::MembershipRequired);
        }
        if let Some(database) = self.database.as_mut() {
            let updated = database
                .execute(
                    "UPDATE campaign_group_memberships SET revoked_at = now() \
                     WHERE campaign_id = $1 AND group_id = $2 AND user_id = $3 \
                       AND revoked_at IS NULL",
                    &[&campaign_id.as_str(), &group_id.as_str(), &user_id.as_str()],
                )
                .map_err(map_postgres_error)?;
            if updated != 1 {
                return Err(IdentityError::MembershipRequired);
            }
        }
        self.group_memberships.remove(&key);
        self.publish_verification_state()
    }
}
