#[test]
fn s09_release_readiness_recognizes_p01_entries_and_fails_closed_on_later_work() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root");
    let report = std::env::temp_dir().join(format!("p00a-readiness-{}.json", std::process::id()));
    let python = if Command::new("python").arg("--version").output().is_ok() {
        "python"
    } else {
        "python3"
    };
    let output = Command::new(python)
        .current_dir(root)
        .args(["scripts/ci/release_readiness.py", "--report"])
        .arg(&report)
        .arg("--require-ready")
        .output()
        .expect("release readiness checker executes");
    assert!(
        !output.status.success(),
        "P01 must not bypass later release blockers"
    );
    let payload = fs::read_to_string(&report).expect("readiness report generated for this run");
    fs::remove_file(report).expect("remove temporary readiness report");
    assert!(payload.contains("\"status\": \"BLOCKED\""));
    assert!(!payload.contains("AUD-002"));
    assert!(!payload.contains("AUD-006"));
    assert!(!payload.contains("AUD-001"));
    assert!(!payload.contains("MISSING_PRODUCT_BINARY"));
    assert!(!payload.contains("MISSING_WEB_ENTRYPOINT"));
    assert!(!payload.contains("MISSING_WEB_SCRIPT"));
    assert!(!payload.contains("NO_PRODUCT_DOCKERFILE"));
    assert!(!payload.contains("PLACEHOLDER_SERVICE"));
    assert!(!payload.contains("MUTABLE_PRODUCT_IMAGE"));
    assert!(payload.contains("MISSING_CURRENT_EVIDENCE"));
}

#[test]
fn s09_prod_provider_security_boundary_is_executable() {
    struct TestKms;
    impl KmsClient for TestKms {
        fn decrypt_secret(&self, _secret_id: &str, _version: u64) -> KernelResult<Vec<u8>> {
            Ok(b"not-used-for-rejected-public-endpoint".to_vec())
        }
    }

    let endpoint = ProviderEndpoint::new(
        "ollama",
        "http://0.0.0.0:11434/v1",
        trpg_platform::deployment_ops::SecretReference::mounted("ollama_credential", 1).unwrap(),
        DeploymentEnvironment::Production,
        "local-model-v1",
        "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
    )
    .unwrap();
    let manager = SecretManager::new(KmsSecretResolver::new(TestKms));
    assert!(
        validate_provider_boundary(&DeploymentEnvironment::Production, &endpoint, &manager)
            .is_err()
    );
}
