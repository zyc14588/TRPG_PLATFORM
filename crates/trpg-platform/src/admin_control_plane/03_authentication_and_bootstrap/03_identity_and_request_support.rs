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
