fn execute_certification(
    request: &CertificationRequestArtifact,
    provider: Arc<HttpModelProvider<MountedFileSecretResolver>>,
    authority: &LocalModelCertificationAuthority,
    certificate_path: &Path,
    state_directory: &Path,
) -> Result<(LocalModelCertificate, PathBuf), CertificationExecutionFailure> {
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
        return Err(CertificationExecutionFailure {
            code: "LOCAL_MODEL_CERTIFICATION_SUITE_FAILED".to_owned(),
            evidence_path: Some(run_evidence_path),
            evidence_sha256: Some(run.evidence_sha256().to_owned()),
        });
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
