/// Computes an assessment only. A caller-created assessment is deliberately
/// not accepted by the AI Keeper gate; only a signed, registry-active
/// `LocalModelCertificate` can cross that boundary.
pub fn certify_local_model(input: &CertificationInput) -> LocalModelLevel {
    if input.json_schema_support
        && input.tool_call_support
        && input.visibility_tests_pass
        && input.prompt_injection_tests_pass
        && input.rules_eval_pass
        && input.latency_ms <= 2_000
    {
        LocalModelLevel::Level4
    } else if input.json_schema_support && input.tool_call_support && input.visibility_tests_pass {
        LocalModelLevel::Level3
    } else if input.json_schema_support || input.tool_call_support {
        LocalModelLevel::Level2
    } else if !input.model_id.trim().is_empty() {
        LocalModelLevel::Level1
    } else {
        LocalModelLevel::Level0
    }
}

impl LocalModelCertificationAuthority {
    /// The legacy constructor cannot establish an independent high-water
    /// witness and therefore fails closed.
    #[deprecated(note = "use new_with_checkpoint with an independent witness store")]
    pub fn new(
        _signing_key_id: impl Into<String>,
        _signing_key: &[u8; 32],
        _registry_path: impl AsRef<Path>,
    ) -> AgentResult<Self> {
        Err(invalid_certification_configuration())
    }

    pub fn new_with_checkpoint(
        signing_key_id: impl Into<String>,
        signing_key: &[u8; 32],
        registry_path: impl AsRef<Path>,
        checkpoint_store: Arc<dyn LedgerCheckpointStore>,
    ) -> AgentResult<Self> {
        let signing_key_id = signing_key_id.into();
        let registry_path = registry_path.as_ref();
        validate_registry_configuration(&signing_key_id, registry_path)?;
        let registry_file = open_or_create_private_file(registry_path)?;
        registry_file
            .sync_all()
            .map_err(|_| invalid_certification_configuration())?;
        sync_parent(registry_path)?;

        let anchor_path = companion_path(registry_path, ".head");
        validate_private_file_if_present(&anchor_path)?;
        let lock_path = companion_path(registry_path, ".lock");
        let lock_file = open_or_create_private_file(&lock_path)?;
        let authority = Self {
            signing_key_id,
            signing_key: Zeroizing::new(*signing_key),
            registry_path: registry_path.to_path_buf(),
            anchor_path,
            ledger_id: ledger_checkpoint_id("local-model-certification", registry_path)
                .map_err(AgentError::Core)?,
            checkpoint_store,
            lock_file,
            observed_head: Mutex::new(None),
        };
        authority.validate_registry()?;
        Ok(authority)
    }

    /// Caller-supplied assessment booleans are not evidence and can never
    /// authorize a certificate.
    #[deprecated(note = "use issue_level4_from_run with runner-produced evidence")]
    pub fn issue_level4(
        &self,
        _input: &CertificationInput,
        _model_artifact_sha256: &str,
        _suite_id: &str,
        _ttl: Duration,
    ) -> AgentResult<LocalModelCertificate> {
        Err(AgentError::LocalModelNotCertifiedForAiKp)
    }

    pub fn issue_level4_from_run(
        &self,
        run: &CompletedCertificationRun,
        ttl: Duration,
    ) -> AgentResult<LocalModelCertificate> {
        if !run.valid_for_issuance() || ttl.is_zero() || ttl > MAX_CERTIFICATE_TTL {
            return Err(AgentError::LocalModelNotCertifiedForAiKp);
        }
        let issued_at_unix_ms = trusted_now_unix_ms()?;
        let expires_at_unix_ms = issued_at_unix_ms
            .checked_add(
                u64::try_from(ttl.as_millis())
                    .map_err(|_| invalid_certification_configuration())?,
            )
            .ok_or_else(invalid_certification_configuration)?;
        self.with_registry_lock(|observed_head| {
            let records = self.read_verified_registry(observed_head)?;
            let mut observed_certificates = std::collections::BTreeSet::new();
            for record in records.iter().rev() {
                if !observed_certificates.insert(record.certificate.certificate_id.as_str()) {
                    continue;
                }
                if record.state == RegistryState::Active
                    && self.certificate_matches_run_scope(&record.certificate, run)
                    && issued_at_unix_ms < record.certificate.expires_at_unix_ms
                {
                    self.verify_certificate_signature(&record.certificate)?;
                    return Ok(record.certificate.clone());
                }
            }

            let mut certificate = LocalModelCertificate {
                certificate_id: certificate_id(
                    &run.manifest.model_id,
                    &run.manifest.model_artifact_sha256,
                    &run.manifest.suite_id,
                    run.evidence_sha256(),
                    issued_at_unix_ms,
                ),
                model_id: run.manifest.model_id.clone(),
                model_artifact_sha256: run.manifest.model_artifact_sha256.clone(),
                suite_id: run.manifest.suite_id.clone(),
                certification_binding: run.binding.clone(),
                level: LocalModelLevel::Level4,
                issued_at_unix_ms,
                expires_at_unix_ms,
                signing_key_id: self.signing_key_id.clone(),
                signature: String::new(),
            };
            certificate.signature = self.certificate_signature(&certificate)?;
            self.append_registry_entry_locked(
                &certificate,
                RegistryState::Active,
                &records,
                observed_head,
            )?;
            Ok(certificate)
        })
    }

    fn certificate_matches_run_scope(
        &self,
        certificate: &LocalModelCertificate,
        run: &CompletedCertificationRun,
    ) -> bool {
        let certificate_binding = &certificate.certification_binding;
        let run_binding = &run.binding;
        certificate.level == LocalModelLevel::Level4
            && certificate.model_id == run.manifest.model_id
            && certificate.model_artifact_sha256 == run.manifest.model_artifact_sha256
            && certificate.suite_id == run.manifest.suite_id
            && certificate_binding.provider_id == run_binding.provider_id
            && certificate_binding.provider_type == run_binding.provider_type
            && certificate_binding.provider_runtime_sha256
                == run_binding.provider_runtime_sha256
            && certificate_binding.suite_id == run_binding.suite_id
            && certificate_binding.suite_version == run_binding.suite_version
            && certificate_binding.suite_sha256 == run_binding.suite_sha256
            && certificate_binding.prompt_set_sha256 == run_binding.prompt_set_sha256
            && certificate_binding.tool_schema_sha256 == run_binding.tool_schema_sha256
            && certificate_binding.ruleset_sha256 == run_binding.ruleset_sha256
            && certificate_binding.policy_sha256 == run_binding.policy_sha256
    }

    pub fn revoke(&self, certificate: &LocalModelCertificate) -> AgentResult<()> {
        self.verify_certificate_signature(certificate)?;
        self.append_registry_entry(certificate, RegistryState::Revoked)
    }

    #[deprecated(note = "use ensure_ai_keeper_provider or ensure_ai_keeper_provider_config")]
    pub fn ensure_ai_keeper_model(
        &self,
        _certificate: &LocalModelCertificate,
        _expected_model_id: &str,
        _expected_artifact_sha256: &str,
    ) -> AgentResult<()> {
        Err(AgentError::LocalModelNotCertifiedForAiKp)
    }

    pub fn ensure_ai_keeper_provider(
        &self,
        certificate: &LocalModelCertificate,
        provider: &dyn ExecutableModelProvider,
    ) -> AgentResult<()> {
        let provider_runtime_sha256 = provider.provider_runtime_sha256();
        self.ensure_ai_keeper_binding(
            certificate,
            provider.provider_id().as_str(),
            provider.provider_type(),
            provider.model_id(),
            provider.model_artifact_sha256(),
            &provider_runtime_sha256,
        )
    }

    pub fn ensure_ai_keeper_provider_config(
        &self,
        certificate: &LocalModelCertificate,
        provider: &ProviderConfig,
    ) -> AgentResult<()> {
        validate_provider_config(provider)?;
        let provider_runtime_sha256 = resolve_provider_runtime_sha256(provider)?;
        self.ensure_ai_keeper_binding(
            certificate,
            provider.provider_id.as_str(),
            provider.provider_type,
            &provider.model_id,
            &provider.model_artifact_sha256,
            &provider_runtime_sha256,
        )
    }

    fn ensure_ai_keeper_binding(
        &self,
        certificate: &LocalModelCertificate,
        provider_id: &str,
        provider_type: ProviderType,
        expected_model_id: &str,
        expected_artifact_sha256: &str,
        provider_runtime_sha256: &str,
    ) -> AgentResult<()> {
        self.verify_certificate_signature(certificate)?;
        let suite = LocalModelCertificationSuite::keeper_v1();
        if certificate.level != LocalModelLevel::Level4
            || certificate.model_id != expected_model_id
            || certificate.model_artifact_sha256 != expected_artifact_sha256
            || !certificate.certification_binding.matches_provider_and_suite(
                provider_id,
                provider_type,
                provider_runtime_sha256,
                &suite,
            )
            || trusted_now_unix_ms()? >= certificate.expires_at_unix_ms
            || self.latest_registry_state(certificate)? != Some(RegistryState::Active)
        {
            return Err(AgentError::LocalModelNotCertifiedForAiKp);
        }
        Ok(())
    }
}
