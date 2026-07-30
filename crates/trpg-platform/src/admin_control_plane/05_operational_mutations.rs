impl AdminControlPlane {
    fn configure_provider(
        &mut self,
        request: &AdminHttpRequest,
    ) -> Result<AdminHttpResponse, AdminControlPlaneError> {
        let actor = self.authenticate_owner(request)?;
        let metadata = mutation_metadata(request)?;
        let parsed: ProviderConfigureRequest = parse_body(request)?;
        let descriptor = format!(
            "{}|{}|{}|{}|{}|{}",
            parsed.provider_type,
            parsed.base_url,
            parsed.model_id,
            parsed.model_artifact_sha256,
            parsed.credential_secret_id,
            parsed.credential_secret_version
        );
        let _lock = StateFileLock::acquire(&self.state_path)?;
        let mut state = read_state_unlocked(&self.state_path)?;
        if let Some(response) =
            replay_response(&state, &metadata, "provider.configure", &descriptor)?
        {
            drop(_lock);
            self.append_audit(
                &actor,
                "provider.configure.replay",
                "provider",
                AuditDecision::Permit,
                &metadata,
            )?;
            return Ok(response);
        }
        ensure_expected_version(&state, &metadata)?;
        let reference = SecretReference::mounted(
            parsed.credential_secret_id.clone(),
            parsed.credential_secret_version,
        )
        .map_err(|_| {
            AdminControlPlaneError::InvalidRequest("ADMIN_PROVIDER_SECRET_REFERENCE_INVALID")
        })?;
        self.secret_manager.register(&reference).map_err(|_| {
            AdminControlPlaneError::InvalidRequest("ADMIN_PROVIDER_SECRET_REGISTRATION_FAILED")
        })?;
        let endpoint = ProviderEndpoint::new(
            parsed.provider_type.clone(),
            parsed.base_url.clone(),
            reference,
            DeploymentEnvironment::Production,
            parsed.model_id.clone(),
            parsed.model_artifact_sha256.clone(),
        )
        .map_err(|_| {
            AdminControlPlaneError::InvalidRequest("ADMIN_PROVIDER_CONFIGURATION_INVALID")
        })?;
        let attestation = validate_provider_boundary(&endpoint, self.secret_manager.as_ref())
            .map_err(|_| {
                AdminControlPlaneError::InvalidRequest("ADMIN_PROVIDER_BOUNDARY_INVALID")
            })?;
        let configuration = AdminProviderConfiguration {
            provider_type: parsed.provider_type,
            base_url: parsed.base_url,
            model_id: parsed.model_id,
            model_artifact_sha256: parsed.model_artifact_sha256,
            credential_secret_id: parsed.credential_secret_id,
            credential_secret_version: parsed.credential_secret_version,
            security_snapshot_digest: attestation.security_snapshot_digest().to_owned(),
        };
        state.provider = Some(configuration);
        let response_fields = BTreeMap::from([(
            "security_snapshot_digest".to_owned(),
            json!(state
                .provider
                .as_ref()
                .map(|provider| provider.security_snapshot_digest.as_str())),
        )]);
        commit_receipt(
            &mut state,
            &metadata,
            "provider.configure",
            descriptor,
            "PROVIDER_CONFIGURED",
            "provider",
            response_fields,
        )?;
        write_state_unlocked(&self.state_path, &state)?;
        drop(_lock);
        self.append_audit(
            &actor,
            "provider.configure",
            "provider",
            AuditDecision::Permit,
            &metadata,
        )?;
        Ok(AdminHttpResponse {
            status: 200,
            body: json!({
                "result": "PROVIDER_CONFIGURED",
                "security_snapshot_digest": state
                    .provider
                    .as_ref()
                    .map(|provider| provider.security_snapshot_digest.as_str()),
                "state_version": state.version
            }),
        })
    }

    fn probe_provider(
        &mut self,
        request: &AdminHttpRequest,
    ) -> Result<AdminHttpResponse, AdminControlPlaneError> {
        let actor = self.authenticate_owner(request)?;
        let metadata = mutation_metadata(request)?;
        ensure_empty_body(request)?;
        let _lock = StateFileLock::acquire(&self.state_path)?;
        let mut state = read_state_unlocked(&self.state_path)?;
        let provider = state.provider.clone().ok_or(
            AdminControlPlaneError::Conflict("ADMIN_PROVIDER_NOT_CONFIGURED"),
        )?;
        let descriptor = provider.security_snapshot_digest.clone();
        if let Some(response) =
            replay_response(&state, &metadata, "provider.probe", &descriptor)?
        {
            drop(_lock);
            self.append_audit(
                &actor,
                "provider.probe.replay",
                "provider",
                AuditDecision::Permit,
                &metadata,
            )?;
            return Ok(response);
        }
        ensure_expected_version(&state, &metadata)?;
        let reference = SecretReference::mounted(
            provider.credential_secret_id.clone(),
            provider.credential_secret_version,
        )
        .map_err(|_| AdminControlPlaneError::Persistence("ADMIN_PROVIDER_STATE_INVALID"))?;
        let credential = self.secret_manager.resolve(&reference).map_err(|_| {
            AdminControlPlaneError::OperationUnavailable(
                "provider credential unavailable".to_owned(),
            )
        })?;
        let evidence = credential
            .expose_utf8_to(|value| self.operations.probe_provider(&provider, value))
            .map_err(|_| {
                AdminControlPlaneError::OperationUnavailable(
                    "provider credential invalid".to_owned(),
                )
            })?
            .map_err(AdminControlPlaneError::OperationUnavailable)?;
        let response_fields = evidence_response_fields(&evidence);
        commit_receipt(
            &mut state,
            &metadata,
            "provider.probe",
            descriptor,
            &evidence.code,
            "provider",
            response_fields,
        )?;
        write_state_unlocked(&self.state_path, &state)?;
        drop(_lock);
        self.append_audit(
            &actor,
            "provider.probe",
            "provider",
            AuditDecision::Permit,
            &metadata,
        )?;
        Ok(evidence_response(evidence, false, state.version))
    }

    fn request_model_certification(
        &mut self,
        request: &AdminHttpRequest,
    ) -> Result<AdminHttpResponse, AdminControlPlaneError> {
        let actor = self.authenticate_owner(request)?;
        let metadata = mutation_metadata(request)?;
        let parsed: AdminModelCertificationRequest = parse_body(request)?;
        validate_short_identifier(&parsed.request_id, "ADMIN_CERTIFICATION_REQUEST_INVALID")?;
        let descriptor = format!(
            "{}|{}|{}",
            parsed.request_id, parsed.model_id, parsed.model_artifact_sha256
        );
        let _lock = StateFileLock::acquire(&self.state_path)?;
        let mut state = read_state_unlocked(&self.state_path)?;
        if let Some(response) =
            replay_response(&state, &metadata, "model.certification.request", &descriptor)?
        {
            drop(_lock);
            self.append_audit(
                &actor,
                "model.certification.request.replay",
                &parsed.request_id,
                AuditDecision::Permit,
                &metadata,
            )?;
            return Ok(response);
        }
        ensure_expected_version(&state, &metadata)?;
        let provider = state.provider.as_ref().ok_or(
            AdminControlPlaneError::Conflict("ADMIN_PROVIDER_NOT_CONFIGURED"),
        )?;
        if provider.model_id != parsed.model_id
            || provider.model_artifact_sha256 != parsed.model_artifact_sha256
        {
            return Err(AdminControlPlaneError::Conflict(
                "ADMIN_CERTIFICATION_MODEL_MISMATCH",
            ));
        }
        let evidence = self
            .operations
            .request_model_certification(&parsed)
            .map_err(AdminControlPlaneError::OperationUnavailable)?;
        let response_fields = evidence_response_fields(&evidence);
        commit_receipt(
            &mut state,
            &metadata,
            "model.certification.request",
            descriptor,
            &evidence.code,
            &parsed.request_id,
            response_fields,
        )?;
        write_state_unlocked(&self.state_path, &state)?;
        drop(_lock);
        self.append_audit(
            &actor,
            "model.certification.request",
            &parsed.request_id,
            AuditDecision::Permit,
            &metadata,
        )?;
        Ok(evidence_response(evidence, false, state.version))
    }

    fn create_backup(
        &mut self,
        request: &AdminHttpRequest,
    ) -> Result<AdminHttpResponse, AdminControlPlaneError> {
        let actor = self.authenticate_owner(request)?;
        let metadata = mutation_metadata(request)?;
        let parsed: AdminBackupRequest = parse_body(request)?;
        validate_short_identifier(&parsed.backup_id, "ADMIN_BACKUP_REQUEST_INVALID")?;
        validate_short_identifier(&parsed.schema_version, "ADMIN_BACKUP_REQUEST_INVALID")?;
        let descriptor = format!("{}|{}", parsed.backup_id, parsed.schema_version);
        let _lock = StateFileLock::acquire(&self.state_path)?;
        let mut state = read_state_unlocked(&self.state_path)?;
        if let Some(response) =
            replay_response(&state, &metadata, "backup.create", &descriptor)?
        {
            drop(_lock);
            self.append_audit(
                &actor,
                "backup.create.replay",
                &parsed.backup_id,
                AuditDecision::Permit,
                &metadata,
            )?;
            return Ok(response);
        }
        ensure_expected_version(&state, &metadata)?;
        let evidence = self
            .operations
            .create_backup(&parsed)
            .map_err(AdminControlPlaneError::OperationUnavailable)?;
        state.last_backup_reference = evidence.artifact_reference.clone();
        let response_fields = evidence_response_fields(&evidence);
        commit_receipt(
            &mut state,
            &metadata,
            "backup.create",
            descriptor,
            &evidence.code,
            &parsed.backup_id,
            response_fields,
        )?;
        write_state_unlocked(&self.state_path, &state)?;
        drop(_lock);
        self.append_audit(
            &actor,
            "backup.create",
            &parsed.backup_id,
            AuditDecision::Permit,
            &metadata,
        )?;
        Ok(evidence_response(evidence, false, state.version))
    }

    fn restore_backup(
        &mut self,
        request: &AdminHttpRequest,
    ) -> Result<AdminHttpResponse, AdminControlPlaneError> {
        let actor = self.authenticate_owner(request)?;
        let metadata = mutation_metadata(request)?;
        let parsed: AdminRestoreRequest = parse_body(request)?;
        validate_restore_request(&parsed)?;
        let descriptor = format!(
            "{}|{}|{}",
            parsed.manifest_path, parsed.expected_schema_version, parsed.safety_point_id
        );
        let _lock = StateFileLock::acquire(&self.state_path)?;
        let mut state = read_state_unlocked(&self.state_path)?;
        if let Some(response) =
            replay_response(&state, &metadata, "backup.restore", &descriptor)?
        {
            drop(_lock);
            self.append_audit(
                &actor,
                "backup.restore.replay",
                &parsed.safety_point_id,
                AuditDecision::Permit,
                &metadata,
            )?;
            return Ok(response);
        }
        ensure_expected_version(&state, &metadata)?;
        let evidence = self
            .operations
            .restore_backup(&parsed)
            .map_err(AdminControlPlaneError::OperationUnavailable)?;
        state.last_restore_reference = evidence.artifact_reference.clone();
        let response_fields = evidence_response_fields(&evidence);
        commit_receipt(
            &mut state,
            &metadata,
            "backup.restore",
            descriptor,
            &evidence.code,
            &parsed.safety_point_id,
            response_fields,
        )?;
        write_state_unlocked(&self.state_path, &state)?;
        drop(_lock);
        self.append_audit(
            &actor,
            "backup.restore",
            &parsed.safety_point_id,
            AuditDecision::Permit,
            &metadata,
        )?;
        Ok(evidence_response(evidence, false, state.version))
    }
}
