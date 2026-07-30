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
        Ok(AdminActor {
            actor_id: authentication.subject_id().as_str().to_owned(),
            authentication_reference,
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
