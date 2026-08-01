impl AdminControlPlane {
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
