#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CertificationEvidenceManifest {
    schema_version: u32,
    runner_version: String,
    request_id: String,
    model_id: String,
    model_artifact_sha256: String,
    provider_id: String,
    provider_type: String,
    provider_runtime_sha256: String,
    suite_id: String,
    suite_version: String,
    suite_sha256: String,
    prompt_set_sha256: String,
    tool_schema_sha256: String,
    ruleset_sha256: String,
    policy_sha256: String,
    started_at_unix_ms: u64,
    completed_at_unix_ms: u64,
    status: CertificationRunStatus,
    cases: Vec<CertificationCaseEvidence>,
}

impl CertificationEvidenceManifest {
    pub const fn status(&self) -> CertificationRunStatus {
        self.status
    }

    pub fn cases(&self) -> &[CertificationCaseEvidence] {
        &self.cases
    }

    pub fn model_id(&self) -> &str {
        &self.model_id
    }

    pub fn provider_runtime_sha256(&self) -> &str {
        &self.provider_runtime_sha256
    }

    pub fn suite_version(&self) -> &str {
        &self.suite_version
    }
}

pub struct CompletedCertificationRun {
    manifest: CertificationEvidenceManifest,
    canonical_manifest: Vec<u8>,
    evidence_sha256: String,
    binding: CertificationBinding,
    level: LocalModelLevel,
}

impl std::fmt::Debug for CompletedCertificationRun {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CompletedCertificationRun")
            .field("manifest", &self.manifest)
            .field("canonical_manifest", &"[REDACTED EVIDENCE MANIFEST]")
            .field("evidence_sha256", &self.evidence_sha256)
            .field("binding", &self.binding)
            .field("level", &self.level)
            .finish()
    }
}

impl CompletedCertificationRun {
    pub fn manifest(&self) -> &CertificationEvidenceManifest {
        &self.manifest
    }

    pub fn canonical_manifest(&self) -> &[u8] {
        &self.canonical_manifest
    }

    pub fn evidence_sha256(&self) -> &str {
        &self.evidence_sha256
    }

    pub const fn level(&self) -> LocalModelLevel {
        self.level
    }

    fn valid_for_issuance(&self) -> bool {
        let kinds = self
            .manifest
            .cases
            .iter()
            .map(|case| case.kind)
            .collect::<BTreeSet<_>>();
        self.level == LocalModelLevel::Level4
            && self.manifest.status == CertificationRunStatus::Passed
            && self.manifest.cases.len() == CertificationCaseKind::ALL.len()
            && kinds == CertificationCaseKind::ALL.into_iter().collect()
            && self
                .manifest
                .cases
                .iter()
                .all(|case| case.status == CertificationCaseStatus::Pass)
            && self.evidence_sha256 == sha256_label(&self.canonical_manifest)
            && self.binding.is_valid()
            && self.binding.provider_id == self.manifest.provider_id
            && self.binding.provider_type == self.manifest.provider_type
            && self.binding.provider_runtime_sha256 == self.manifest.provider_runtime_sha256
            && self.binding.suite_id == self.manifest.suite_id
            && self.binding.suite_version == self.manifest.suite_version
            && self.binding.suite_sha256 == self.manifest.suite_sha256
            && self.binding.prompt_set_sha256 == self.manifest.prompt_set_sha256
            && self.binding.tool_schema_sha256 == self.manifest.tool_schema_sha256
            && self.binding.ruleset_sha256 == self.manifest.ruleset_sha256
            && self.binding.policy_sha256 == self.manifest.policy_sha256
            && self.binding.evidence_sha256 == self.evidence_sha256
    }
}

pub struct LocalModelCertificationRunner {
    provider: Arc<dyn ExecutableModelProvider>,
    suite: LocalModelCertificationSuite,
    case_timeout: Duration,
    maximum_probe_retries: u32,
}

impl std::fmt::Debug for LocalModelCertificationRunner {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LocalModelCertificationRunner")
            .field("provider", &"[MODEL PROVIDER ADAPTER]")
            .field("suite", &self.suite)
            .field("case_timeout", &self.case_timeout)
            .field("maximum_probe_retries", &self.maximum_probe_retries)
            .finish()
    }
}

impl LocalModelCertificationRunner {
    pub fn new(
        provider: Arc<dyn ExecutableModelProvider>,
        suite: LocalModelCertificationSuite,
        case_timeout: Duration,
    ) -> AgentResult<Self> {
        if case_timeout.is_zero() || case_timeout > suite.maximum_case_timeout {
            return Err(invalid_certification_configuration());
        }
        Ok(Self {
            provider,
            suite,
            case_timeout,
            maximum_probe_retries: 1,
        })
    }

    pub async fn run(
        &self,
        request: &CertificationRequest,
        cancellation: &ProviderCancellation,
    ) -> AgentResult<CompletedCertificationRun> {
        self.validate_request(request)?;
        let started_at_unix_ms = trusted_now_unix_ms()?;
        let mut cases = Vec::with_capacity(CertificationCaseKind::ALL.len());
        for kind in CertificationCaseKind::ALL {
            cases.push(if cancellation.is_cancelled() {
                self.not_run_evidence(kind)
            } else {
                self.execute_case(kind, cancellation).await
            });
        }
        let completed_at_unix_ms = trusted_now_unix_ms()?;
        let status = if cases
            .iter()
            .all(|case| case.status == CertificationCaseStatus::Pass)
        {
            CertificationRunStatus::Passed
        } else if cases
            .iter()
            .any(|case| case.status == CertificationCaseStatus::NotRun)
        {
            CertificationRunStatus::Partial
        } else {
            CertificationRunStatus::Failed
        };
        let manifest = CertificationEvidenceManifest {
            schema_version: CERTIFICATION_EVIDENCE_SCHEMA_VERSION,
            runner_version: CERTIFICATION_RUNNER_VERSION.to_owned(),
            request_id: request.request_id.clone(),
            model_id: request.model_id.clone(),
            model_artifact_sha256: request.model_artifact_sha256.clone(),
            provider_id: request.provider_id.clone(),
            provider_type: request.provider_type.route_name().to_owned(),
            provider_runtime_sha256: request.provider_runtime_sha256.clone(),
            suite_id: self.suite.suite_id.clone(),
            suite_version: self.suite.suite_version.clone(),
            suite_sha256: self.suite.suite_sha256.clone(),
            prompt_set_sha256: self.suite.prompt_set_sha256.clone(),
            tool_schema_sha256: self.suite.tool_schema_sha256.clone(),
            ruleset_sha256: self.suite.ruleset_sha256.clone(),
            policy_sha256: self.suite.policy_sha256.clone(),
            started_at_unix_ms,
            completed_at_unix_ms,
            status,
            cases,
        };
        let mut canonical_manifest = serde_json::to_vec_pretty(&manifest)
            .map_err(|_| invalid_certification_configuration())?;
        canonical_manifest.push(b'\n');
        let evidence_sha256 = sha256_label(&canonical_manifest);
        let binding = CertificationBinding {
            provider_id: manifest.provider_id.clone(),
            provider_type: manifest.provider_type.clone(),
            provider_runtime_sha256: manifest.provider_runtime_sha256.clone(),
            suite_id: manifest.suite_id.clone(),
            suite_version: manifest.suite_version.clone(),
            suite_sha256: manifest.suite_sha256.clone(),
            prompt_set_sha256: manifest.prompt_set_sha256.clone(),
            tool_schema_sha256: manifest.tool_schema_sha256.clone(),
            ruleset_sha256: manifest.ruleset_sha256.clone(),
            policy_sha256: manifest.policy_sha256.clone(),
            evidence_sha256: evidence_sha256.clone(),
        };
        Ok(CompletedCertificationRun {
            manifest,
            canonical_manifest,
            evidence_sha256,
            binding,
            level: if status == CertificationRunStatus::Passed {
                LocalModelLevel::Level4
            } else {
                LocalModelLevel::Level3
            },
        })
    }

    fn validate_request(&self, request: &CertificationRequest) -> AgentResult<()> {
        let startup = self.provider.startup_route_snapshot();
        if request.suite_id != self.suite.suite_id
            || request.suite_version != self.suite.suite_version
            || request.provider_id != self.provider.provider_id().as_str()
            || request.provider_type != self.provider.provider_type()
            || request.model_id != self.provider.model_id()
            || request.model_artifact_sha256 != self.provider.model_artifact_sha256()
            || request.provider_runtime_sha256 != self.provider.provider_runtime_sha256()
            || !request.provider_type.is_local()
            || startup.provider_id != *self.provider.provider_id()
            || startup.provider_type != request.provider_type
            || startup.model_id != request.model_id
            || startup.operation != crate::model_provider::ModelOperation::CapabilityProbe
        {
            return Err(AgentError::LocalModelNotCertifiedForAiKp);
        }
        Ok(())
    }

    fn not_run_evidence(&self, kind: CertificationCaseKind) -> CertificationCaseEvidence {
        CertificationCaseEvidence {
            kind,
            status: CertificationCaseStatus::NotRun,
            request_summary: kind.as_str().to_owned(),
            redacted_request: "[NO PROVIDER REQUEST]".to_owned(),
            request_sha256: sha256_label(kind.as_str().as_bytes()),
            response_summary: "not_run".to_owned(),
            redacted_response: "[NO PROVIDER RESPONSE]".to_owned(),
            response_sha256: None,
            latency_ms: 0,
            retry_count: 0,
            error_code: Some("certification_cancelled".to_owned()),
        }
    }
}
