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

}
