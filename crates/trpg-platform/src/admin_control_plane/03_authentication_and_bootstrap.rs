impl AdminControlPlane {
    fn complete_bootstrap(
        &mut self,
        request: &AdminHttpRequest,
    ) -> Result<AdminHttpResponse, AdminControlPlaneError> {
        let metadata = mutation_metadata(request)?;
        let parsed: BootstrapCompleteRequest = parse_body(request)?;
        validate_account_request(&parsed.administrator)?;
        validate_account_request(&parsed.business_account)?;
        if parsed.administrator.user_id == parsed.business_account.user_id
            || parsed.administrator.login.eq_ignore_ascii_case(&parsed.business_account.login)
        {
            return Err(AdminControlPlaneError::InvalidRequest(
                "BOOTSTRAP_ACCOUNTS_MUST_BE_DISTINCT",
            ));
        }
        let bearer = bearer_token(request)?;
        if !constant_time_secret_matches(&self.bootstrap_token, bearer.as_bytes()) {
            return Err(AdminControlPlaneError::Authentication(
                "BOOTSTRAP_TOKEN_INVALID",
            ));
        }

        let _lock = StateFileLock::acquire(&self.state_path)?;
        let mut state = read_state_unlocked(&self.state_path)?;
        if state.bootstrap_token_consumed {
            return Err(AdminControlPlaneError::Authentication(
                "BOOTSTRAP_TOKEN_CONSUMED",
            ));
        }
        ensure_expected_version(&state, &metadata)?;
        ensure_identity_user(
            &mut self.identity,
            &parsed.administrator,
            GlobalRole::ServerOwner,
        )?;
        ensure_identity_user(
            &mut self.identity,
            &parsed.business_account,
            GlobalRole::User,
        )?;
        state.bootstrap_token_consumed = true;
        state.administrator_user_id = Some(parsed.administrator.user_id.clone());
        state.administrator_login = Some(parsed.administrator.login.clone());
        state.business_user_id = Some(parsed.business_account.user_id.clone());
        state.business_login = Some(parsed.business_account.login.clone());
        state.version = state
            .version
            .checked_add(1)
            .ok_or(AdminControlPlaneError::Persistence(
                "ADMIN_STATE_VERSION_OVERFLOW",
            ))?;
        insert_receipt(
            &mut state,
            &metadata,
            "bootstrap.complete",
            format!(
                "{}|{}",
                parsed.administrator.user_id, parsed.business_account.user_id
            ),
            "BOOTSTRAP_COMPLETED",
            "bootstrap",
            BTreeMap::new(),
        )?;
        write_state_unlocked(&self.state_path, &state)?;
        drop(_lock);
        self.append_audit(
            &AdminActor {
                actor_id: "bootstrap-token".to_owned(),
                authentication_reference: "one-time-bootstrap-secret".to_owned(),
            },
            "bootstrap.complete",
            "bootstrap",
            AuditDecision::Permit,
            &metadata,
        )?;
        Ok(AdminHttpResponse {
            status: 201,
            body: json!({
                "status": "completed",
                "state_version": state.version,
                "administrator_user_id": state.administrator_user_id,
                "business_user_id": state.business_user_id,
                "bootstrap_token_consumed": true
            }),
        })
    }

    fn create_session(
        &mut self,
        request: &AdminHttpRequest,
    ) -> Result<AdminHttpResponse, AdminControlPlaneError> {
        let parsed: SessionLoginRequest = parse_body(request)?;
        let now = now_unix_ms()?;
        let session = self
            .identity
            .login(&parsed.login, &parsed.password, now)
            .map_err(map_authentication_error)?;
        let authentication = self
            .identity
            .authenticate_session(Some(session.token.expose()), now)
            .map_err(map_authentication_error)?;
        if !matches!(
            authentication.kind(),
            PrincipalKind::UserSession {
                global_role: GlobalRole::ServerOwner,
                ..
            }
        ) {
            return Err(AdminControlPlaneError::Authorization(
                "ADMIN_SERVER_OWNER_REQUIRED",
            ));
        }
        Ok(AdminHttpResponse {
            status: 200,
            body: json!({
                "token_type": "Bearer",
                "access_token": session.token.expose(),
                "expires_at_unix_ms": session.expires_at_unix_ms
            }),
        })
    }

    fn authenticate_owner(
        &mut self,
        request: &AdminHttpRequest,
    ) -> Result<AdminActor, AdminControlPlaneError> {
        self.authenticate_owner_context(request)
            .map(|(actor, _)| actor)
    }

    fn authenticate_owner_context(
        &mut self,
        request: &AdminHttpRequest,
    ) -> Result<(AdminActor, AuthenticationContext), AdminControlPlaneError> {
        let bearer = bearer_token(request)?;
        let authentication = self
            .identity
            .authenticate_session(Some(bearer), now_unix_ms()?)
            .map_err(map_authentication_error)?;
        let authentication_reference = match authentication.kind() {
            PrincipalKind::UserSession {
                session_id,
                global_role: GlobalRole::ServerOwner,
            } => session_id.as_str().to_owned(),
            PrincipalKind::UserSession { .. } => {
                return Err(AdminControlPlaneError::Authorization(
                    "ADMIN_SERVER_OWNER_REQUIRED",
                ))
            }
            _ => {
                return Err(AdminControlPlaneError::Authorization(
                    "ADMIN_USER_SESSION_REQUIRED",
                ))
            }
        };
        Ok((
            AdminActor {
                actor_id: authentication.subject_id().as_str().to_owned(),
                authentication_reference,
            },
            authentication,
        ))
    }

    fn configure_tutorial_authority(
        &mut self,
        request: &AdminHttpRequest,
    ) -> Result<AdminHttpResponse, AdminControlPlaneError> {
        let (actor, authentication) = self.authenticate_owner_context(request)?;
        let metadata = mutation_metadata(request)?;
        let parsed: BootstrapTutorialAuthorityRequest = parse_body(request)?;
        let descriptor = format!(
            "{}|{}|{}|{}|{}",
            parsed.campaign_id,
            parsed.contract_id,
            parsed.created_at_unix_ms,
            parsed.ai_provider_snapshot,
            parsed.model_route_snapshot
        );
        let _lock = StateFileLock::acquire(&self.state_path)?;
        let mut state = read_state_unlocked(&self.state_path)?;
        if let Some(response) = replay_response(
            &state,
            &metadata,
            "bootstrap.tutorial_authority",
            &descriptor,
        )? {
            drop(_lock);
            self.append_audit(
                &actor,
                "bootstrap.tutorial_authority.replay",
                &parsed.campaign_id,
                AuditDecision::Permit,
                &metadata,
            )?;
            return Ok(response);
        }
        ensure_expected_version(&state, &metadata)?;
        if !state.bootstrap_token_consumed {
            return Err(AdminControlPlaneError::Conflict(
                "ADMIN_BOOTSTRAP_NOT_COMPLETED",
            ));
        }
        let business_user_id = state.business_user_id.clone().ok_or(
            AdminControlPlaneError::Persistence("ADMIN_BUSINESS_ACCOUNT_MISSING"),
        )?;
        let contract = tutorial_authority_contract(&parsed, &business_user_id)?;
        let campaign_id = contract.campaign_id().clone();
        let now = now_unix_ms()?;
        match self
            .identity
            .authority_contract(&campaign_id)
            .map_err(map_tutorial_identity_error)?
        {
            Some(existing) if existing != contract => {
                return Err(AdminControlPlaneError::Conflict(
                    "TUTORIAL_AUTHORITY_CONTRACT_CONFLICT",
                ));
            }
            Some(_) => {}
            None => {
                self.identity
                    .grant_membership(
                        &authentication,
                        parsed.campaign_id.clone(),
                        business_user_id.clone(),
                        CampaignRole::HumanKeeper,
                        now,
                    )
                    .map_err(map_tutorial_identity_error)?;
                self.identity
                    .register_authority_contract(
                        &authentication,
                        contract,
                        now,
                    )
                    .map_err(map_tutorial_identity_error)?;
            }
        }
        self.operations
            .provision_workflow_policy(campaign_id.as_str())
            .map_err(AdminControlPlaneError::OperationUnavailable)?;
        let response_fields = BTreeMap::from([
            ("campaign_id".to_owned(), json!(parsed.campaign_id)),
            ("contract_id".to_owned(), json!(parsed.contract_id)),
            ("authority_owner".to_owned(), json!(business_user_id)),
            ("authority_mode".to_owned(), json!("HUMAN_KP")),
            ("ruleset_version".to_owned(), json!(TUTORIAL_RULESET_VERSION)),
            (
                "scenario_version".to_owned(),
                json!(TUTORIAL_SCENARIO_VERSION),
            ),
        ]);
        commit_receipt(
            &mut state,
            &metadata,
            "bootstrap.tutorial_authority",
            descriptor,
            "TUTORIAL_AUTHORITY_CONFIGURED",
            campaign_id.as_str(),
            response_fields,
        )?;
        write_state_unlocked(&self.state_path, &state)?;
        drop(_lock);
        self.append_audit(
            &actor,
            "bootstrap.tutorial_authority",
            campaign_id.as_str(),
            AuditDecision::Permit,
            &metadata,
        )?;
        Ok(AdminHttpResponse {
            status: 201,
            body: json!({
                "result": "TUTORIAL_AUTHORITY_CONFIGURED",
                "campaign_id": campaign_id.as_str(),
                "contract_id": parsed.contract_id,
                "authority_owner": state.business_user_id,
                "authority_mode": "HUMAN_KP",
                "ruleset_version": TUTORIAL_RULESET_VERSION,
                "scenario_version": TUTORIAL_SCENARIO_VERSION,
                "state_version": state.version,
            }),
        })
    }

    fn bootstrap_status(
        &mut self,
        request: &AdminHttpRequest,
    ) -> Result<AdminHttpResponse, AdminControlPlaneError> {
        let state = self.load_state()?;
        if state.bootstrap_token_consumed {
            self.authenticate_owner(request)?;
        } else {
            let bearer = bearer_token(request)?;
            if !constant_time_secret_matches(&self.bootstrap_token, bearer.as_bytes()) {
                return Err(AdminControlPlaneError::Authentication(
                    "BOOTSTRAP_TOKEN_INVALID",
                ));
            }
        }
        Ok(AdminHttpResponse {
            status: 200,
            body: json!({
                "status": if state.bootstrap_token_consumed { "completed" } else { "pending" },
                "state_version": state.version,
                "bootstrap_token_consumed": state.bootstrap_token_consumed,
                "administrator_count": usize::from(state.administrator_user_id.is_some()),
                "business_account_count": usize::from(state.business_user_id.is_some()),
                "provider_configured": state.provider.is_some()
            }),
        })
    }

    fn create_managed_user(
        &mut self,
        request: &AdminHttpRequest,
    ) -> Result<AdminHttpResponse, AdminControlPlaneError> {
        let (actor, _) = self.authenticate_owner_context(request)?;
        let metadata = mutation_metadata(request)?;
        let parsed: CreateUserRequest = parse_body(request)?;
        validate_account_request(&BootstrapAccountRequest {
            user_id: parsed.user_id.clone(),
            login: parsed.login.clone(),
            password: parsed.password.clone(),
        })?;
        let descriptor = format!("{}|{}", parsed.user_id, parsed.login.to_ascii_lowercase());
        let _lock = StateFileLock::acquire(&self.state_path)?;
        let mut state = read_state_unlocked(&self.state_path)?;
        if let Some(response) = replay_response(
            &state,
            &metadata,
            "identity.user.create",
            &descriptor,
        )? {
            drop(_lock);
            self.append_audit(
                &actor,
                "identity.user.create.replay",
                &parsed.user_id,
                AuditDecision::Permit,
                &metadata,
            )?;
            return Ok(response);
        }
        ensure_expected_version(&state, &metadata)?;
        if !state.bootstrap_token_consumed {
            return Err(AdminControlPlaneError::Conflict(
                "ADMIN_BOOTSTRAP_NOT_COMPLETED",
            ));
        }
        ensure_managed_user(&mut self.identity, &parsed)?;
        let response_fields = BTreeMap::from([
            ("user_id".to_owned(), json!(parsed.user_id)),
            ("global_role".to_owned(), json!("USER")),
        ]);
        commit_receipt(
            &mut state,
            &metadata,
            "identity.user.create",
            descriptor,
            "USER_CREATED",
            &parsed.user_id,
            response_fields,
        )?;
        write_state_unlocked(&self.state_path, &state)?;
        drop(_lock);
        self.append_audit(
            &actor,
            "identity.user.create",
            &parsed.user_id,
            AuditDecision::Permit,
            &metadata,
        )?;
        Ok(AdminHttpResponse {
            status: 201,
            body: json!({
                "result": "USER_CREATED",
                "user_id": parsed.user_id,
                "global_role": "USER",
                "state_version": state.version,
            }),
        })
    }

    fn fork_authority(
        &mut self,
        request: &AdminHttpRequest,
    ) -> Result<AdminHttpResponse, AdminControlPlaneError> {
        let (actor, authentication) = self.authenticate_owner_context(request)?;
        let metadata = mutation_metadata(request)?;
        let parsed: ForkAuthorityRequest = parse_body(request)?;
        let parent_campaign_id = EntityId::new(&parsed.parent_campaign_id).map_err(|_| {
            AdminControlPlaneError::InvalidRequest("AUTHORITY_FORK_REQUEST_INVALID")
        })?;
        EntityId::new(&parsed.child_campaign_id)
            .and_then(|_| EntityId::new(&parsed.authority_owner))
            .and_then(|_| EntityId::new(&parsed.campaign_manager_user_id))
            .map_err(|_| {
                AdminControlPlaneError::InvalidRequest("AUTHORITY_FORK_REQUEST_INVALID")
            })?;
        if parsed.parent_campaign_id == parsed.child_campaign_id {
            return Err(AdminControlPlaneError::InvalidRequest(
                "AUTHORITY_FORK_REQUEST_INVALID",
            ));
        }
        let authority_mode = match parsed.authority_mode.as_str() {
            "AI_KP" => AuthorityMode::AiKp,
            "HUMAN_KP" if parsed.authority_owner == parsed.campaign_manager_user_id => {
                AuthorityMode::HumanKp
            }
            _ => {
                return Err(AdminControlPlaneError::InvalidRequest(
                    "AUTHORITY_FORK_REQUEST_INVALID",
                ))
            }
        };
        let descriptor = format!(
            "{}|{}|{}|{}|{}",
            parsed.parent_campaign_id,
            parsed.child_campaign_id,
            parsed.authority_mode,
            parsed.authority_owner,
            parsed.campaign_manager_user_id,
        );
        let _lock = StateFileLock::acquire(&self.state_path)?;
        let mut state = read_state_unlocked(&self.state_path)?;
        if let Some(response) = replay_response(
            &state,
            &metadata,
            "authority.fork",
            &descriptor,
        )? {
            drop(_lock);
            self.append_audit(
                &actor,
                "authority.fork.replay",
                &parsed.child_campaign_id,
                AuditDecision::Permit,
                &metadata,
            )?;
            return Ok(response);
        }
        ensure_expected_version(&state, &metadata)?;
        if !state.bootstrap_token_consumed {
            return Err(AdminControlPlaneError::Conflict(
                "ADMIN_BOOTSTRAP_NOT_COMPLETED",
            ));
        }
        let parent = self
            .identity
            .authority_contract(&parent_campaign_id)
            .map_err(map_authority_fork_identity_error)?
            .ok_or(AdminControlPlaneError::Conflict(
                "AUTHORITY_FORK_PARENT_NOT_FOUND",
            ))?;
        let child = parent
            .fork_for_child(
                parsed.child_campaign_id.clone(),
                authority_mode.clone(),
                parsed.authority_owner.clone(),
            )
            .map_err(|_| {
                AdminControlPlaneError::InvalidRequest("AUTHORITY_FORK_REQUEST_INVALID")
            })?;
        let now = now_unix_ms()?;
        let child_campaign_id = child.campaign_id().clone();
        match self
            .identity
            .authority_contract(&child_campaign_id)
            .map_err(map_authority_fork_identity_error)?
        {
            Some(existing) if existing != child => {
                return Err(AdminControlPlaneError::Conflict(
                    "AUTHORITY_FORK_CHILD_CONFLICT",
                ));
            }
            Some(_) => {}
            None => {
                let manager_role = if authority_mode == AuthorityMode::AiKp {
                    CampaignRole::CampaignOwner
                } else {
                    CampaignRole::HumanKeeper
                };
                self.identity
                    .grant_membership(
                        &authentication,
                        parsed.child_campaign_id.clone(),
                        parsed.campaign_manager_user_id.clone(),
                        manager_role,
                        now,
                    )
                    .map_err(map_authority_fork_identity_error)?;
                self.identity
                    .register_authority_contract(&authentication, child.clone(), now)
                    .map_err(map_authority_fork_identity_error)?;
            }
        }
        self.operations
            .provision_workflow_policy(child_campaign_id.as_str())
            .map_err(AdminControlPlaneError::OperationUnavailable)?;
        let mode_name = if authority_mode == AuthorityMode::AiKp {
            "AI_KP"
        } else {
            "HUMAN_KP"
        };
        let response_fields = BTreeMap::from([
            ("parent_campaign_id".to_owned(), json!(parsed.parent_campaign_id)),
            ("child_campaign_id".to_owned(), json!(parsed.child_campaign_id)),
            ("contract_id".to_owned(), json!(child.contract_id().as_str())),
            ("authority_mode".to_owned(), json!(mode_name)),
            ("authority_owner".to_owned(), json!(parsed.authority_owner)),
            (
                "campaign_manager_user_id".to_owned(),
                json!(parsed.campaign_manager_user_id),
            ),
        ]);
        commit_receipt(
            &mut state,
            &metadata,
            "authority.fork",
            descriptor,
            "AUTHORITY_FORKED",
            child_campaign_id.as_str(),
            response_fields,
        )?;
        write_state_unlocked(&self.state_path, &state)?;
        drop(_lock);
        self.append_audit(
            &actor,
            "authority.fork",
            child_campaign_id.as_str(),
            AuditDecision::Permit,
            &metadata,
        )?;
        Ok(AdminHttpResponse {
            status: 201,
            body: json!({
                "result": "AUTHORITY_FORKED",
                "parent_campaign_id": parsed.parent_campaign_id,
                "child_campaign_id": parsed.child_campaign_id,
                "contract_id": child.contract_id().as_str(),
                "authority_mode": mode_name,
                "authority_owner": parsed.authority_owner,
                "campaign_manager_user_id": parsed.campaign_manager_user_id,
                "state_version": state.version,
            }),
        })
    }
}

fn tutorial_authority_contract(
    request: &BootstrapTutorialAuthorityRequest,
    business_user_id: &str,
) -> Result<AuthorityContract, AdminControlPlaneError> {
    AuthorityContract::new_locked(AuthorityContractDraft {
        contract_id: request.contract_id.clone(),
        campaign_id: request.campaign_id.clone(),
        mode: AuthorityMode::HumanKp,
        authority_owner: business_user_id.to_owned(),
        version: 1,
        snapshot: AuthorityVersionSnapshotDraft {
            ruleset_version: TUTORIAL_RULESET_VERSION.to_owned(),
            house_rules_version: TUTORIAL_HOUSE_RULES_VERSION.to_owned(),
            scenario_version: TUTORIAL_SCENARIO_VERSION.to_owned(),
            prompt_version: TUTORIAL_PROMPT_VERSION.to_owned(),
            agent_pack_version: TUTORIAL_AGENT_PACK_VERSION.to_owned(),
            tool_schema_version: TUTORIAL_TOOL_SCHEMA_VERSION.to_owned(),
            safety_profile_version: TUTORIAL_SAFETY_PROFILE_VERSION.to_owned(),
            ai_provider_snapshot: request.ai_provider_snapshot.clone(),
            model_route_snapshot: request.model_route_snapshot.clone(),
            character_sheet_template_version: TUTORIAL_CHARACTER_TEMPLATE_VERSION.to_owned(),
        },
        created_at_unix_ms: request.created_at_unix_ms,
    })
    .map_err(|_| {
        AdminControlPlaneError::InvalidRequest("TUTORIAL_AUTHORITY_CONTRACT_INVALID")
    })
}

fn map_tutorial_identity_error(error: IdentityError) -> AdminControlPlaneError {
    match error {
        IdentityError::MembershipDenied | IdentityError::AuthorityContractConflict => {
            AdminControlPlaneError::Conflict("TUTORIAL_AUTHORITY_CONTRACT_CONFLICT")
        }
        _ => AdminControlPlaneError::Persistence("TUTORIAL_AUTHORITY_PERSISTENCE_FAILED"),
    }
}

fn ensure_managed_user(
    identity: &mut IdentityService,
    account: &CreateUserRequest,
) -> Result<(), AdminControlPlaneError> {
    match identity.create_user(
        account.user_id.clone(),
        &account.login,
        &account.password,
        GlobalRole::User,
    ) {
        Ok(()) => Ok(()),
        Err(IdentityError::DuplicateLogin) => {
            let now = now_unix_ms()?;
            let session = identity
                .login(&account.login, &account.password, now)
                .map_err(|_| {
                    AdminControlPlaneError::Conflict("ADMIN_EXISTING_USER_CONFLICT")
                })?;
            let token = session.token.expose().to_owned();
            let authentication = identity
                .authenticate_session(Some(&token), now)
                .map_err(|_| {
                    AdminControlPlaneError::Conflict("ADMIN_EXISTING_USER_CONFLICT")
                })?;
            let matches = authentication.subject_id().as_str() == account.user_id
                && matches!(
                    authentication.kind(),
                    PrincipalKind::UserSession {
                        global_role: GlobalRole::User,
                        ..
                    }
                );
            identity.logout(&token).map_err(|_| {
                AdminControlPlaneError::Persistence("ADMIN_USER_SESSION_REVOKE_FAILED")
            })?;
            if matches {
                Ok(())
            } else {
                Err(AdminControlPlaneError::Conflict(
                    "ADMIN_EXISTING_USER_CONFLICT",
                ))
            }
        }
        Err(_) => Err(AdminControlPlaneError::Persistence(
            "ADMIN_USER_CREATION_FAILED",
        )),
    }
}

fn map_authority_fork_identity_error(error: IdentityError) -> AdminControlPlaneError {
    match error {
        IdentityError::MembershipDenied => {
            AdminControlPlaneError::Conflict("AUTHORITY_FORK_MEMBERSHIP_CONFLICT")
        }
        IdentityError::AuthorityContractConflict => {
            AdminControlPlaneError::Conflict("AUTHORITY_FORK_CHILD_CONFLICT")
        }
        IdentityError::InvalidIdentityData => {
            AdminControlPlaneError::InvalidRequest("AUTHORITY_FORK_REQUEST_INVALID")
        }
        _ => AdminControlPlaneError::Persistence("AUTHORITY_FORK_PERSISTENCE_FAILED"),
    }
}

fn ensure_identity_user(
    identity: &mut IdentityService,
    account: &BootstrapAccountRequest,
    expected_role: GlobalRole,
) -> Result<(), AdminControlPlaneError> {
    match identity.create_user(
        account.user_id.clone(),
        &account.login,
        &account.password,
        expected_role,
    ) {
        Ok(()) | Err(IdentityError::DuplicateLogin) => {}
        Err(_) => {
            return Err(AdminControlPlaneError::Persistence(
                "BOOTSTRAP_ACCOUNT_CREATION_FAILED",
            ))
        }
    }
    let now = now_unix_ms()?;
    let session = identity
        .login(&account.login, &account.password, now)
        .map_err(|_| {
            AdminControlPlaneError::Conflict("BOOTSTRAP_EXISTING_ACCOUNT_CONFLICT")
        })?;
    let authentication = identity
        .authenticate_session(Some(session.token.expose()), now)
        .map_err(|_| {
            AdminControlPlaneError::Conflict("BOOTSTRAP_EXISTING_ACCOUNT_CONFLICT")
        })?;
    if !matches!(
        authentication.kind(),
        PrincipalKind::UserSession { global_role, .. } if *global_role == expected_role
    ) {
        return Err(AdminControlPlaneError::Conflict(
            "BOOTSTRAP_EXISTING_ACCOUNT_ROLE_CONFLICT",
        ));
    }
    Ok(())
}

fn validate_account_request(
    account: &BootstrapAccountRequest,
) -> Result<(), AdminControlPlaneError> {
    if account.user_id.trim().is_empty()
        || account.user_id.len() > 128
        || account.login.trim().is_empty()
        || account.login.len() > 254
        || account.password.len() < 16
        || account.password.len() > 1024
    {
        return Err(AdminControlPlaneError::InvalidRequest(
            "BOOTSTRAP_ACCOUNT_INVALID",
        ));
    }
    Ok(())
}

fn bearer_token(request: &AdminHttpRequest) -> Result<&str, AdminControlPlaneError> {
    request
        .header("authorization")
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|value| !value.is_empty() && value.len() <= 4096)
        .ok_or(AdminControlPlaneError::Authentication(
            "ADMIN_AUTHENTICATION_REQUIRED",
        ))
}

fn mutation_metadata(
    request: &AdminHttpRequest,
) -> Result<MutationMetadata, AdminControlPlaneError> {
    let required = |name: &str| {
        request
            .header(name)
            .filter(|value| !value.trim().is_empty() && value.len() <= 256)
            .map(str::to_owned)
            .ok_or(AdminControlPlaneError::InvalidRequest(
                "ADMIN_MUTATION_METADATA_REQUIRED",
            ))
    };
    let expected_version = required("x-expected-version")?
        .parse::<u64>()
        .map_err(|_| {
            AdminControlPlaneError::InvalidRequest("ADMIN_EXPECTED_VERSION_INVALID")
        })?;
    Ok(MutationMetadata {
        idempotency_key: required("idempotency-key")?,
        expected_version,
        correlation_id: required("x-correlation-id")?,
        causation_id: required("x-causation-id")?,
    })
}

fn parse_body<T: for<'de> Deserialize<'de>>(
    request: &AdminHttpRequest,
) -> Result<T, AdminControlPlaneError> {
    if request.body.is_empty() || request.body.len() > MAX_REQUEST_BODY_BYTES {
        return Err(AdminControlPlaneError::InvalidRequest(
            "ADMIN_REQUEST_BODY_INVALID",
        ));
    }
    serde_json::from_slice(&request.body)
        .map_err(|_| AdminControlPlaneError::InvalidRequest("ADMIN_REQUEST_SCHEMA_INVALID"))
}

fn constant_time_secret_matches(secret: &SecretValue, candidate: &[u8]) -> bool {
    secret.expose_to(|expected| {
        let mut different = expected.len() ^ candidate.len();
        let length = expected.len().max(candidate.len());
        for index in 0..length {
            let left = expected.get(index).copied().unwrap_or_default();
            let right = candidate.get(index).copied().unwrap_or_default();
            different |= usize::from(left ^ right);
        }
        different == 0
    })
}

fn map_authentication_error(error: IdentityError) -> AdminControlPlaneError {
    match error {
        IdentityError::PersistenceUnavailable => {
            AdminControlPlaneError::Persistence("ADMIN_IDENTITY_UNAVAILABLE")
        }
        _ => AdminControlPlaneError::Authentication("ADMIN_INVALID_CREDENTIALS"),
    }
}
