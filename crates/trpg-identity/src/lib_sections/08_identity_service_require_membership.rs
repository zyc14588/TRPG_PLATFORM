
impl IdentityService {

    pub fn require_membership(
        &mut self,
        actor: &AuthenticationContext,
        campaign_id: &EntityId,
        allowed: &[CampaignRole],
        now_unix_ms: u64,
    ) -> Result<CampaignMembership, IdentityError> {
        self.sync_if_persistent()?;
        self.verifier().verify(actor, now_unix_ms)?;
        actor.require_campaign(campaign_id)?;
        let membership = self
            .memberships
            .get(&(campaign_id.clone(), actor.subject_id.clone()))
            .ok_or(IdentityError::MembershipRequired)?;
        if !allowed.contains(&membership.role) {
            return Err(IdentityError::MembershipDenied);
        }
        Ok(membership.clone())
    }

    pub fn membership_for(
        &mut self,
        actor: &AuthenticationContext,
        campaign_id: &EntityId,
        now_unix_ms: u64,
    ) -> Result<Option<CampaignMembership>, IdentityError> {
        self.sync_if_persistent()?;
        self.verifier().verify(actor, now_unix_ms)?;
        actor.require_campaign(campaign_id)?;
        Ok(self
            .memberships
            .get(&(campaign_id.clone(), actor.subject_id.clone()))
            .cloned())
    }

    pub fn require_membership_manager(
        &mut self,
        actor: &AuthenticationContext,
        campaign_id: &EntityId,
        now_unix_ms: u64,
    ) -> Result<Option<CampaignMembership>, IdentityError> {
        self.sync_if_persistent()?;
        self.verifier().verify(actor, now_unix_ms)?;
        if matches!(
            actor.kind,
            PrincipalKind::UserSession {
                global_role: GlobalRole::ServerOwner,
                ..
            }
        ) {
            return Ok(None);
        }
        let membership = self
            .memberships
            .get(&(campaign_id.clone(), actor.subject_id.clone()))
            .filter(|membership| membership.role == CampaignRole::CampaignOwner)
            .cloned()
            .ok_or(IdentityError::MembershipDenied)?;
        Ok(Some(membership))
    }

    pub fn command_actor(
        &mut self,
        authentication: &AuthenticationContext,
        campaign_id: &EntityId,
        now_unix_ms: u64,
    ) -> Result<Actor, IdentityError> {
        self.sync_if_persistent()?;
        self.verifier().verify(authentication, now_unix_ms)?;
        authentication.require_campaign(campaign_id)?;
        let membership = match authentication.kind() {
            PrincipalKind::UserSession {
                global_role: GlobalRole::User,
                ..
            } => Some(
                self.memberships
                    .get(&(campaign_id.clone(), authentication.subject_id.clone()))
                    .ok_or(IdentityError::MembershipRequired)?,
            ),
            PrincipalKind::UserSession { .. }
            | PrincipalKind::Workload { .. }
            | PrincipalKind::AgentRun { .. } => None,
        };
        authentication.to_command_actor(membership)
    }

    pub fn register_authority_contract(
        &mut self,
        actor: &AuthenticationContext,
        contract: AuthorityContract,
        now_unix_ms: u64,
    ) -> Result<(), IdentityError> {
        self.sync_if_persistent()?;
        self.verifier().verify(actor, now_unix_ms)?;
        let server_owner = matches!(
            actor.kind,
            PrincipalKind::UserSession {
                global_role: GlobalRole::ServerOwner,
                ..
            }
        );
        let campaign_owner = self
            .memberships
            .get(&(contract.campaign_id().clone(), actor.subject_id.clone()))
            .is_some_and(|membership| membership.role == CampaignRole::CampaignOwner);
        if !server_owner && !campaign_owner {
            return Err(IdentityError::MembershipDenied);
        }
        if self.authorities.contains_key(contract.campaign_id()) {
            return Err(IdentityError::AuthorityContractConflict);
        }
        if contract.mode() == &AuthorityMode::HumanKp {
            let owner_membership = self.memberships.get(&(
                contract.campaign_id().clone(),
                contract.authority_owner().clone(),
            ));
            if !owner_membership
                .is_some_and(|membership| membership.role == CampaignRole::HumanKeeper)
            {
                return Err(IdentityError::MembershipDenied);
            }
            if self.memberships.values().any(|membership| {
                membership.campaign_id == *contract.campaign_id()
                    && membership.user_id != *contract.authority_owner()
                    && membership.role == CampaignRole::HumanKeeper
            }) {
                return Err(IdentityError::MembershipDenied);
            }
        } else if self.memberships.values().any(|membership| {
            membership.campaign_id == *contract.campaign_id()
                && membership.role == CampaignRole::HumanKeeper
        }) {
            return Err(IdentityError::MembershipDenied);
        }

        if let Some(database) = self.database.as_mut() {
            let version = i64::try_from(contract.version())
                .map_err(|_| IdentityError::InvalidIdentityData)?;
            let created_at = i64::try_from(contract.created_at_unix_ms())
                .map_err(|_| IdentityError::InvalidIdentityData)?;
            let snapshot = contract.snapshot();
            database
                .execute(
                    "INSERT INTO authority_contracts (\
                        contract_id, campaign_id, authority_mode, authority_owner, \
                        contract_version, ruleset_version, house_rules_version, \
                        scenario_version, prompt_version, agent_pack_version, \
                        tool_schema_version, safety_profile_version, ai_provider_snapshot, \
                        model_route_snapshot, character_sheet_template_version, created_at\
                     ) VALUES (\
                        $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, \
                        to_timestamp($16::bigint / 1000.0)\
                     )",
                    &[
                        &contract.contract_id().as_str(),
                        &contract.campaign_id().as_str(),
                        &authority_mode_name(contract.mode()),
                        &contract.authority_owner().as_str(),
                        &version,
                        &snapshot.ruleset_version().as_str(),
                        &snapshot.house_rules_version().as_str(),
                        &snapshot.scenario_version().as_str(),
                        &snapshot.prompt_version().as_str(),
                        &snapshot.agent_pack_version().as_str(),
                        &snapshot.tool_schema_version().as_str(),
                        &snapshot.safety_profile_version().as_str(),
                        &snapshot.ai_provider_snapshot().as_str(),
                        &snapshot.model_route_snapshot().as_str(),
                        &snapshot.character_sheet_template_version().as_str(),
                        &created_at,
                    ],
                )
                .map_err(map_postgres_error)?;
        }
        self.authorities
            .insert(contract.campaign_id().clone(), contract);
        self.publish_verification_state()
    }

    pub fn authority_contract(
        &mut self,
        campaign_id: &EntityId,
    ) -> Result<Option<AuthorityContract>, IdentityError> {
        self.sync_if_persistent()?;
        Ok(self.authorities.get(campaign_id).cloned())
    }

    pub fn issue_workload_credential(
        &self,
        workload_id: &str,
        role: WorkloadRole,
        issued_at_unix_ms: u64,
        expires_at_unix_ms: u64,
    ) -> Result<String, IdentityError> {
        let workload_id =
            EntityId::new(workload_id).map_err(|_| IdentityError::InvalidIdentityData)?;
        if expires_at_unix_ms <= issued_at_unix_ms {
            return Err(IdentityError::InvalidIdentityData);
        }
        let claims = format!(
            "{}|workload|{}|{}|{}|{}",
            INTERNAL_TOKEN_VERSION,
            workload_id,
            role.as_str(),
            issued_at_unix_ms,
            expires_at_unix_ms
        );
        Ok(self.sign_claims(&claims))
    }

    pub fn authenticate_workload(
        &self,
        credential: &str,
        now_unix_ms: u64,
    ) -> Result<AuthenticationContext, IdentityError> {
        let claims = self.verify_signed_credential(credential)?;
        let fields = claims.split('|').collect::<Vec<_>>();
        if fields.len() != 6 || fields[0] != INTERNAL_TOKEN_VERSION || fields[1] != "workload" {
            return Err(IdentityError::InvalidInternalCredential);
        }
        let subject_id =
            EntityId::new(fields[2]).map_err(|_| IdentityError::InvalidInternalCredential)?;
        let role = WorkloadRole::parse(fields[3])?;
        let issued_at_unix_ms = parse_timestamp(fields[4])?;
        let expires_at_unix_ms = parse_timestamp(fields[5])?;
        ensure_internal_time(now_unix_ms, issued_at_unix_ms, expires_at_unix_ms)?;
        Ok(AuthenticationContext {
            subject_id,
            kind: PrincipalKind::Workload { role },
            authenticated_at_unix_ms: issued_at_unix_ms,
            expires_at_unix_ms,
            issuer_fingerprint: self.verifier().issuer_fingerprint,
        })
    }

    pub fn issue_agent_run_credential(
        &self,
        run_id: &str,
        agent_id: &str,
        campaign_id: &str,
        class: AgentClass,
        issued_at_unix_ms: u64,
        expires_at_unix_ms: u64,
    ) -> Result<String, IdentityError> {
        let run_id = EntityId::new(run_id).map_err(|_| IdentityError::InvalidIdentityData)?;
        let agent_id = EntityId::new(agent_id).map_err(|_| IdentityError::InvalidIdentityData)?;
        let campaign_id =
            EntityId::new(campaign_id).map_err(|_| IdentityError::InvalidIdentityData)?;
        if expires_at_unix_ms <= issued_at_unix_ms {
            return Err(IdentityError::InvalidIdentityData);
        }
        let claims = format!(
            "{}|agent|{}|{}|{}|{}|{}|{}",
            INTERNAL_TOKEN_VERSION,
            run_id,
            agent_id,
            campaign_id,
            class.as_str(),
            issued_at_unix_ms,
            expires_at_unix_ms
        );
        Ok(self.sign_claims(&claims))
    }

    pub fn authenticate_agent_run(
        &self,
        credential: &str,
        now_unix_ms: u64,
    ) -> Result<AuthenticationContext, IdentityError> {
        let claims = self.verify_signed_credential(credential)?;
        let fields = claims.split('|').collect::<Vec<_>>();
        if fields.len() != 8 || fields[0] != INTERNAL_TOKEN_VERSION || fields[1] != "agent" {
            return Err(IdentityError::InvalidInternalCredential);
        }
        let run_id =
            EntityId::new(fields[2]).map_err(|_| IdentityError::InvalidInternalCredential)?;
        let subject_id =
            EntityId::new(fields[3]).map_err(|_| IdentityError::InvalidInternalCredential)?;
        let campaign_id =
            EntityId::new(fields[4]).map_err(|_| IdentityError::InvalidInternalCredential)?;
        let class = AgentClass::parse(fields[5])?;
        let issued_at_unix_ms = parse_timestamp(fields[6])?;
        let expires_at_unix_ms = parse_timestamp(fields[7])?;
        ensure_internal_time(now_unix_ms, issued_at_unix_ms, expires_at_unix_ms)?;
        Ok(AuthenticationContext {
            subject_id,
            kind: PrincipalKind::AgentRun {
                run_id,
                class,
                campaign_id,
            },
            authenticated_at_unix_ms: issued_at_unix_ms,
            expires_at_unix_ms,
            issuer_fingerprint: self.verifier().issuer_fingerprint,
        })
    }

    fn issue_session(
        &mut self,
        user_id: EntityId,
        now_unix_ms: u64,
    ) -> Result<LoginSession, IdentityError> {
        let (token_hash, record, session) = self.generate_session(user_id, now_unix_ms)?;
        if let Some(database) = self.database.as_mut() {
            persist_session(database, token_hash, &record, None)?;
        }
        self.sessions_by_hash.insert(token_hash, record);
        self.publish_verification_state()?;
        Ok(session)
    }

    fn generate_session(
        &self,
        user_id: EntityId,
        now_unix_ms: u64,
    ) -> Result<([u8; 32], SessionRecord, LoginSession), IdentityError> {
        let mut raw_token = [0_u8; SESSION_TOKEN_BYTES];
        OsRng.fill_bytes(&mut raw_token);
        let token = SessionToken(hex_encode(&raw_token));
        let token_hash = hash_token(token.expose());
        let mut raw_session_id = [0_u8; 16];
        OsRng.fill_bytes(&mut raw_session_id);
        let session_id = EntityId::new(format!("session_{}", hex_encode(&raw_session_id)))
            .map_err(|_| IdentityError::InvalidIdentityData)?;
        let expires_at_unix_ms = now_unix_ms
            .checked_add(self.session_ttl_ms)
            .ok_or(IdentityError::InvalidIdentityData)?;
        let record = SessionRecord {
            session_id,
            user_id,
            issued_at_unix_ms: now_unix_ms,
            expires_at_unix_ms,
            revoked: false,
        };
        let session = LoginSession {
            token,
            expires_at_unix_ms,
        };
        Ok((token_hash, record, session))
    }

    fn sign_claims(&self, claims: &str) -> String {
        let mut mac = HmacSha256::new_from_slice(&self.signing_key)
            .expect("identity key length was validated at construction");
        mac.update(claims.as_bytes());
        format!("{claims}.{}", hex_encode(&mac.finalize().into_bytes()))
    }
}
