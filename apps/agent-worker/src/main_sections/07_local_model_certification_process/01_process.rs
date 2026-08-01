use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

const CERTIFICATION_PROCESS_SCHEMA_VERSION: u32 = 1;
const CERTIFICATION_TTL: Duration = Duration::from_secs(30 * 24 * 60 * 60);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AgentWorkerStartupMode {
    Ready,
    CertificationOnly,
}

impl AgentWorkerStartupMode {
    fn from_environment() -> Result<Self, String> {
        match std::env::var("TRPG_AGENT_WORKER_MODE") {
            Err(std::env::VarError::NotPresent) => Self::parse(None),
            Err(std::env::VarError::NotUnicode(_)) => {
                Err("TRPG_AGENT_WORKER_MODE_INVALID".to_owned())
            }
            Ok(value) => Self::parse(Some(&value)),
        }
    }

    fn parse(value: Option<&str>) -> Result<Self, String> {
        match value {
            None | Some("ready") => Ok(Self::Ready),
            Some("certification-only") => Ok(Self::CertificationOnly),
            Some(_) => Err("TRPG_AGENT_WORKER_MODE_INVALID".to_owned()),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct CertificationRequestArtifact {
    request_id: String,
    model_id: String,
    model_artifact_sha256: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum CertificationProcessState {
    Claimed,
    Succeeded,
    Failed,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CertificationProcessRecord {
    schema_version: u32,
    request_id: String,
    state: CertificationProcessState,
    attempt: u32,
    claim_owner: String,
    model_id: String,
    model_artifact_sha256: String,
    provider_id: String,
    provider_type: String,
    provider_runtime_sha256: String,
    certificate_path: Option<String>,
    certificate_id: Option<String>,
    evidence_path: Option<String>,
    evidence_sha256: Option<String>,
    error_code: Option<String>,
}

impl CertificationProcessRecord {
    fn for_provider(
        request: &CertificationRequestArtifact,
        provider: &dyn ExecutableModelProvider,
        state: CertificationProcessState,
        attempt: u32,
    ) -> Self {
        Self {
            schema_version: CERTIFICATION_PROCESS_SCHEMA_VERSION,
            request_id: request.request_id.clone(),
            state,
            attempt,
            claim_owner: "local-model-certifier".to_owned(),
            model_id: request.model_id.clone(),
            model_artifact_sha256: request.model_artifact_sha256.clone(),
            provider_id: provider.provider_id().as_str().to_owned(),
            provider_type: provider.provider_type().route_name().to_owned(),
            provider_runtime_sha256: provider.provider_runtime_sha256(),
            certificate_path: None,
            certificate_id: None,
            evidence_path: None,
            evidence_sha256: None,
            error_code: None,
        }
    }

    fn matches(
        &self,
        request: &CertificationRequestArtifact,
        provider: &dyn ExecutableModelProvider,
    ) -> bool {
        self.schema_version == CERTIFICATION_PROCESS_SCHEMA_VERSION
            && self.request_id == request.request_id
            && self.model_id == request.model_id
            && self.model_artifact_sha256 == request.model_artifact_sha256
            && self.provider_id == provider.provider_id().as_str()
            && self.provider_type == provider.provider_type().route_name()
            && self.provider_runtime_sha256 == provider.provider_runtime_sha256()
    }
}

fn run_local_model_certification_from_environment() -> Result<(), String> {
    let result = run_local_model_certification_inner();
    if let Err(error) = &result {
        let _ = persist_terminal_failure_if_possible(error);
    }
    result
}

fn run_local_model_certification_inner() -> Result<(), String> {
    let request_id = required_environment("TRPG_LOCAL_MODEL_CERTIFICATION_REQUEST_ID")?;
    validate_certification_identifier(&request_id)?;
    let request_directory = PathBuf::from(required_environment(
        "TRPG_LOCAL_MODEL_CERTIFICATION_REQUEST_DIRECTORY",
    )?);
    validate_absolute_directory(&request_directory)?;
    let request_path = request_directory.join(format!("{request_id}.request"));
    validate_regular_absolute_file(&request_path)?;
    let request = decode_certification_request(
        &fs::read(&request_path)
            .map_err(|_| "LOCAL_MODEL_CERTIFICATION_REQUEST_UNREADABLE".to_owned())?,
    )?;
    if request.request_id != request_id {
        return Err("LOCAL_MODEL_CERTIFICATION_REQUEST_ID_MISMATCH".to_owned());
    }

    let certificate_path =
        PathBuf::from(required_environment("TRPG_LOCAL_MODEL_CERTIFICATE_PATH")?);
    let registry_path = PathBuf::from(required_environment(
        "TRPG_LOCAL_MODEL_CERTIFICATION_REGISTRY_PATH",
    )?);
    let state_directory = certificate_path
        .parent()
        .ok_or_else(|| "LOCAL_MODEL_CERTIFICATION_STATE_PATH_INVALID".to_owned())?;
    if !certificate_path.is_absolute()
        || !registry_path.is_absolute()
        || registry_path.parent() != Some(state_directory)
    {
        return Err("LOCAL_MODEL_CERTIFICATION_STATE_PATH_INVALID".to_owned());
    }
    ensure_private_directory(state_directory)?;
    let _process_lock = acquire_certification_process_lock(state_directory)?;
    reject_symlink_if_present(&certificate_path)?;
    reject_symlink_if_present(&registry_path)?;
    let result_path = state_directory.join(format!("{}.result.json", request.request_id));
    reject_symlink_if_present(&result_path)?;

    let secret_manager = Arc::new(production_secret_manager()?);
    let provider = Arc::new(model_provider_from_environment(Arc::clone(&secret_manager))?);
    if provider.provider_type() == ProviderType::Cloud {
        return Err("LOCAL_MODEL_CERTIFICATION_REQUIRES_LOCAL_PROVIDER".to_owned());
    }
    if provider.model_id() != request.model_id
        || provider.model_artifact_sha256() != request.model_artifact_sha256
    {
        return Err("LOCAL_MODEL_CERTIFICATION_REQUEST_MODEL_MISMATCH".to_owned());
    }
    let authority = certification_authority_from_environment(
        &secret_manager,
        registry_path,
    )?;

    let existing = read_process_record(&result_path)?;
    if let Some(record) = &existing {
        if !record.matches(&request, provider.as_ref()) {
            return Err("LOCAL_MODEL_CERTIFICATION_RESULT_IDENTITY_MISMATCH".to_owned());
        }
        match record.state {
            CertificationProcessState::Succeeded => {
                recover_certificate(
                    &authority,
                    provider.as_ref(),
                    &certificate_path,
                    state_directory,
                    record,
                )?;
                return Ok(());
            }
            CertificationProcessState::Failed => {
                return Err(record
                    .error_code
                    .clone()
                    .unwrap_or_else(|| "LOCAL_MODEL_CERTIFICATION_TERMINAL_FAILURE".to_owned()));
            }
            CertificationProcessState::Claimed => {}
        }
    }
    let attempt = existing
        .as_ref()
        .map_or(Ok(1), |record| record.attempt.checked_add(1).ok_or(()))
        .map_err(|()| "LOCAL_MODEL_CERTIFICATION_ATTEMPT_OVERFLOW".to_owned())?;

    if certificate_path.is_file() {
        let mut recovered = CertificationProcessRecord::for_provider(
            &request,
            provider.as_ref(),
            CertificationProcessState::Succeeded,
            attempt,
        );
        populate_recovered_record(
            &authority,
            provider.as_ref(),
            &certificate_path,
            state_directory,
            &mut recovered,
        )?;
        write_process_record(&result_path, &recovered)?;
        return Ok(());
    }

    let claimed = CertificationProcessRecord::for_provider(
        &request,
        provider.as_ref(),
        CertificationProcessState::Claimed,
        attempt,
    );
    write_process_record(&result_path, &claimed)?;
    match execute_certification(
        &request,
        Arc::clone(&provider),
        &authority,
        &certificate_path,
        state_directory,
    ) {
        Ok((certificate, evidence_path)) => {
            let mut succeeded = CertificationProcessRecord::for_provider(
                &request,
                provider.as_ref(),
                CertificationProcessState::Succeeded,
                attempt,
            );
            succeeded.certificate_path = Some(certificate_path.display().to_string());
            succeeded.certificate_id = Some(certificate.certificate_id().to_owned());
            succeeded.evidence_path = Some(evidence_path.display().to_string());
            succeeded.evidence_sha256 = Some(
                certificate
                    .certification_binding()
                    .evidence_sha256()
                    .to_owned(),
            );
            write_process_record(&result_path, &succeeded)?;
            println!(
                "service=agent-worker mode=certification-only request_id={} result=succeeded attempt={attempt}",
                request.request_id
            );
            Ok(())
        }
        Err(error) => {
            let mut failed = CertificationProcessRecord::for_provider(
                &request,
                provider.as_ref(),
                CertificationProcessState::Failed,
                attempt,
            );
            failed.error_code = Some(error.clone());
            write_process_record(&result_path, &failed)?;
            Err(error)
        }
    }
}

fn execute_certification(
    request: &CertificationRequestArtifact,
    provider: Arc<HttpModelProvider<MountedFileSecretResolver>>,
    authority: &LocalModelCertificationAuthority,
    certificate_path: &Path,
    state_directory: &Path,
) -> Result<(LocalModelCertificate, PathBuf), String> {
    let suite = LocalModelCertificationSuite::keeper_v1();
    let provider_runtime_sha256 = provider.provider_runtime_sha256();
    let certification_request = CertificationRequest::new(
        &request.request_id,
        &request.model_id,
        &request.model_artifact_sha256,
        provider.provider_id().as_str(),
        provider.provider_type(),
        &provider_runtime_sha256,
        suite.suite_id(),
        suite.suite_version(),
    )
    .map_err(|error| error.code().to_owned())?;
    let timeout = Duration::from_millis(bounded_environment_u64(
        "TRPG_MODEL_PROVIDER_TIMEOUT_MS",
        30_000,
        1,
        30_000,
    )?);
    let executable: Arc<dyn ExecutableModelProvider> = provider.clone();
    let runner = LocalModelCertificationRunner::new(executable, suite, timeout)
        .map_err(|error| error.code().to_owned())?;
    let runtime = tokio::runtime::Runtime::new()
        .map_err(|_| "LOCAL_MODEL_CERTIFICATION_RUNTIME_INITIALIZATION_FAILED".to_owned())?;
    let run = runtime
        .block_on(runner.run(
            &certification_request,
            &ProviderCancellation::default(),
        ))
        .map_err(|error| error.code().to_owned())?;
    let run_evidence_path = evidence_path(state_directory, run.evidence_sha256())?;
    write_private_once(&run_evidence_path, run.canonical_manifest())?;
    if run.manifest().status() != CertificationRunStatus::Passed {
        return Err("LOCAL_MODEL_CERTIFICATION_SUITE_FAILED".to_owned());
    }

    let certificate = authority
        .issue_level4_from_run(&run, CERTIFICATION_TTL)
        .map_err(|error| error.code().to_owned())?;
    authority
        .ensure_ai_keeper_provider(&certificate, provider.as_ref())
        .map_err(|error| error.code().to_owned())?;
    let selected_evidence_path = evidence_path(
        state_directory,
        certificate.certification_binding().evidence_sha256(),
    )?;
    validate_regular_absolute_file(&selected_evidence_path)
        .map_err(|_| "LOCAL_MODEL_CERTIFICATION_EVIDENCE_MISSING".to_owned())?;
    let mut encoded = serde_json::to_vec_pretty(&certificate)
        .map_err(|_| "LOCAL_MODEL_CERTIFICATE_SERIALIZATION_FAILED".to_owned())?;
    encoded.push(b'\n');
    write_private_once(certificate_path, &encoded)?;
    Ok((certificate, selected_evidence_path))
}
