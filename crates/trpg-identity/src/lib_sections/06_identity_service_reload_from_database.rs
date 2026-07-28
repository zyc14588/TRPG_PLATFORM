
impl IdentityService {

    fn reload_from_database(&mut self) -> Result<(), IdentityError> {
        let database = self
            .database
            .as_mut()
            .ok_or(IdentityError::PersistenceUnavailable)?;

        let mut users_by_login = HashMap::new();
        let mut users_by_id = HashMap::new();
        for row in database
            .query(
                "SELECT user_id, login_normalized, password_hash, global_role \
                   FROM users WHERE disabled_at IS NULL",
                &[],
            )
            .map_err(|_| IdentityError::PersistenceUnavailable)?
        {
            let record = UserRecord {
                user_id: EntityId::new(row.get::<_, String>(0))
                    .map_err(|_| IdentityError::InvalidIdentityData)?,
                password_hash: row.get(2),
                global_role: parse_global_role(row.get::<_, String>(3).as_str())?,
            };
            users_by_login.insert(row.get(1), record.clone());
            users_by_id.insert(record.user_id.clone(), record);
        }

        let mut sessions_by_hash = HashMap::new();
        for row in database
            .query(
                "SELECT session_id, user_id, token_hash, \
                        (extract(epoch FROM issued_at) * 1000)::bigint, \
                        (extract(epoch FROM expires_at) * 1000)::bigint, \
                        revoked_at IS NOT NULL \
                   FROM sessions",
                &[],
            )
            .map_err(|_| IdentityError::PersistenceUnavailable)?
        {
            let token_hash = row.get::<_, Vec<u8>>(2);
            let token_hash: [u8; 32] = token_hash
                .try_into()
                .map_err(|_| IdentityError::InvalidIdentityData)?;
            sessions_by_hash.insert(
                token_hash,
                SessionRecord {
                    session_id: EntityId::new(row.get::<_, String>(0))
                        .map_err(|_| IdentityError::InvalidIdentityData)?,
                    user_id: EntityId::new(row.get::<_, String>(1))
                        .map_err(|_| IdentityError::InvalidIdentityData)?,
                    issued_at_unix_ms: timestamp_from_i64(row.get(3))?,
                    expires_at_unix_ms: timestamp_from_i64(row.get(4))?,
                    revoked: row.get(5),
                },
            );
        }

        let mut memberships = HashMap::new();
        for row in database
            .query(
                "SELECT campaign_id, user_id, role \
                   FROM campaign_memberships WHERE revoked_at IS NULL",
                &[],
            )
            .map_err(|_| IdentityError::PersistenceUnavailable)?
        {
            let membership = CampaignMembership {
                campaign_id: EntityId::new(row.get::<_, String>(0))
                    .map_err(|_| IdentityError::InvalidIdentityData)?,
                user_id: EntityId::new(row.get::<_, String>(1))
                    .map_err(|_| IdentityError::InvalidIdentityData)?,
                role: parse_campaign_role(row.get::<_, String>(2).as_str())?,
            };
            memberships.insert(
                (membership.campaign_id.clone(), membership.user_id.clone()),
                membership,
            );
        }

        let mut campaign_groups = HashMap::new();
        for row in database
            .query("SELECT campaign_id, group_id FROM campaign_groups", &[])
            .map_err(|_| IdentityError::PersistenceUnavailable)?
        {
            let group = CampaignGroup {
                campaign_id: EntityId::new(row.get::<_, String>(0))
                    .map_err(|_| IdentityError::InvalidIdentityData)?,
                group_id: EntityId::new(row.get::<_, String>(1))
                    .map_err(|_| IdentityError::InvalidIdentityData)?,
            };
            campaign_groups.insert((group.campaign_id.clone(), group.group_id.clone()), group);
        }

        let mut group_memberships = HashMap::new();
        for row in database
            .query(
                "SELECT campaign_id, group_id, user_id \
                   FROM campaign_group_memberships WHERE revoked_at IS NULL",
                &[],
            )
            .map_err(|_| IdentityError::PersistenceUnavailable)?
        {
            let membership = CampaignGroupMembership {
                campaign_id: EntityId::new(row.get::<_, String>(0))
                    .map_err(|_| IdentityError::InvalidIdentityData)?,
                group_id: EntityId::new(row.get::<_, String>(1))
                    .map_err(|_| IdentityError::InvalidIdentityData)?,
                user_id: EntityId::new(row.get::<_, String>(2))
                    .map_err(|_| IdentityError::InvalidIdentityData)?,
            };
            group_memberships.insert(
                (
                    membership.campaign_id.clone(),
                    membership.group_id.clone(),
                    membership.user_id.clone(),
                ),
                membership,
            );
        }

        let mut authorities = HashMap::new();
        for row in database
            .query(
                "SELECT contract_id, campaign_id, authority_mode, authority_owner, \
                        contract_version, ruleset_version, house_rules_version, \
                        scenario_version, prompt_version, agent_pack_version, \
                        tool_schema_version, safety_profile_version, ai_provider_snapshot, \
                        model_route_snapshot, character_sheet_template_version, \
                        (extract(epoch FROM created_at) * 1000)::bigint \
                   FROM authority_contracts",
                &[],
            )
            .map_err(|_| IdentityError::PersistenceUnavailable)?
        {
            let version = u64::try_from(row.get::<_, i64>(4))
                .map_err(|_| IdentityError::InvalidIdentityData)?;
            let contract = AuthorityContract::new_locked(AuthorityContractDraft {
                contract_id: row.get(0),
                campaign_id: row.get(1),
                mode: parse_authority_mode(row.get::<_, String>(2).as_str())?,
                authority_owner: row.get(3),
                version,
                snapshot: AuthorityVersionSnapshotDraft {
                    ruleset_version: row.get(5),
                    house_rules_version: row.get(6),
                    scenario_version: row.get(7),
                    prompt_version: row.get(8),
                    agent_pack_version: row.get(9),
                    tool_schema_version: row.get(10),
                    safety_profile_version: row.get(11),
                    ai_provider_snapshot: row.get(12),
                    model_route_snapshot: row.get(13),
                    character_sheet_template_version: row.get(14),
                },
                created_at_unix_ms: timestamp_from_i64(row.get(15))?,
            })
            .map_err(|_| IdentityError::InvalidIdentityData)?;
            authorities.insert(contract.campaign_id().clone(), contract);
        }

        self.users_by_login = users_by_login;
        self.users_by_id = users_by_id;
        self.sessions_by_hash = sessions_by_hash;
        self.memberships = memberships;
        self.campaign_groups = campaign_groups;
        self.group_memberships = group_memberships;
        self.authorities = authorities;
        self.publish_verification_state()
    }

    fn sync_if_persistent(&mut self) -> Result<(), IdentityError> {
        if self.database.is_some() {
            self.reload_from_database()?;
        }
        Ok(())
    }

    fn can_manage_campaign_memberships(
        &self,
        actor: &AuthenticationContext,
        campaign_id: &EntityId,
    ) -> bool {
        matches!(
            actor.kind,
            PrincipalKind::UserSession {
                global_role: GlobalRole::ServerOwner,
                ..
            }
        ) || self
            .memberships
            .get(&(campaign_id.clone(), actor.subject_id.clone()))
            .is_some_and(|membership| membership.role == CampaignRole::CampaignOwner)
    }

    pub fn create_user(
        &mut self,
        user_id: impl Into<String>,
        login: &str,
        password: &str,
        global_role: GlobalRole,
    ) -> Result<(), IdentityError> {
        self.sync_if_persistent()?;
        let normalized_login = normalize_login(login)?;
        if self.users_by_login.contains_key(&normalized_login) {
            return Err(IdentityError::DuplicateLogin);
        }
        validate_password(password)?;
        let user_id = EntityId::new(user_id).map_err(|_| IdentityError::InvalidIdentityData)?;
        let salt = SaltString::generate(&mut OsRng);
        let password_hash = Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map_err(|_| IdentityError::PasswordHashFailure)?
            .to_string();
        let record = UserRecord {
            user_id: user_id.clone(),
            password_hash,
            global_role,
        };
        if let Some(database) = self.database.as_mut() {
            database
                .execute(
                    "INSERT INTO users \
                        (user_id, login_normalized, password_hash, global_role) \
                     VALUES ($1, $2, $3, $4)",
                    &[
                        &record.user_id.as_str(),
                        &normalized_login,
                        &record.password_hash,
                        &global_role_name(record.global_role),
                    ],
                )
                .map_err(map_postgres_error)?;
        }
        self.users_by_login.insert(normalized_login, record.clone());
        self.users_by_id.insert(user_id, record);
        Ok(())
    }

    pub fn login(
        &mut self,
        login: &str,
        password: &str,
        now_unix_ms: u64,
    ) -> Result<LoginSession, IdentityError> {
        self.sync_if_persistent()?;
        let normalized_login = normalize_login(login).ok();
        let rate_limit_key = login_rate_limit_key(normalized_login.as_deref().unwrap_or(login));
        self.reserve_login_attempt(&rate_limit_key, now_unix_ms)?;

        let user = normalized_login
            .as_ref()
            .and_then(|normalized| self.users_by_login.get(normalized));
        let password_hash = user
            .map(|record| record.password_hash.as_str())
            .unwrap_or(self.dummy_password_hash.as_str());
        let password_matches = {
            let _permit = self.password_verification_gate.try_acquire()?;
            verify_password(password, password_hash)?
        };
        let user_id = match (user, password_matches) {
            (Some(user), true) => user.user_id.clone(),
            _ => {
                self.record_login_failure(rate_limit_key, now_unix_ms);
                return Err(IdentityError::InvalidCredentials);
            }
        };

        self.clear_login_attempts(&rate_limit_key)?;
        self.issue_session(user_id, now_unix_ms)
    }

    fn reserve_login_attempt(&mut self, key: &str, now_unix_ms: u64) -> Result<(), IdentityError> {
        if let Some(security) = &self.distributed_login_security {
            security.reserve(key)
        } else {
            self.ensure_login_not_rate_limited(key, now_unix_ms)
        }
    }

    fn clear_login_attempts(&mut self, key: &str) -> Result<(), IdentityError> {
        if let Some(security) = &self.distributed_login_security {
            security.clear(key)
        } else {
            self.login_attempts.remove(key);
            Ok(())
        }
    }

    fn ensure_login_not_rate_limited(
        &mut self,
        key: &str,
        now_unix_ms: u64,
    ) -> Result<(), IdentityError> {
        let Some(attempt) = self.login_attempts.get_mut(key) else {
            return Ok(());
        };
        if now_unix_ms < attempt.blocked_until_unix_ms {
            return Err(IdentityError::LoginRateLimited);
        }
        if now_unix_ms.saturating_sub(attempt.window_started_at_unix_ms) >= LOGIN_FAILURE_WINDOW_MS
        {
            self.login_attempts.remove(key);
        }
        Ok(())
    }

    fn record_login_failure(&mut self, key: String, now_unix_ms: u64) {
        if self.distributed_login_security.is_some() {
            return;
        }
        let attempt = self.login_attempts.entry(key).or_insert(LoginAttemptState {
            window_started_at_unix_ms: now_unix_ms,
            failures: 0,
            blocked_until_unix_ms: 0,
        });
        if now_unix_ms.saturating_sub(attempt.window_started_at_unix_ms) >= LOGIN_FAILURE_WINDOW_MS
        {
            attempt.window_started_at_unix_ms = now_unix_ms;
            attempt.failures = 0;
            attempt.blocked_until_unix_ms = 0;
        }
        attempt.failures = attempt.failures.saturating_add(1);
        if attempt.failures >= LOGIN_FAILURE_LIMIT {
            attempt.blocked_until_unix_ms = now_unix_ms.saturating_add(LOGIN_BLOCK_MS);
        }
    }
}
