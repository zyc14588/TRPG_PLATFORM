use std::{fs, path::Path, process::Command};

use trpg_platform::deployment_ops::{
    validate_provider_boundary, DeploymentEnvironment, KmsClient, KmsSecretResolver,
    ProviderEndpoint, SecretManager,
};
use trpg_shared_kernel::KernelResult;

const COMPOSE: &str = include_str!("../../../compose.yml");
const CI_COMPOSE: &str = include_str!("../../../docker-compose.ci.yml");
const DOCKERFILE: &str = include_str!("../../../Dockerfile");
const DEV_SMOKE: &str = include_str!("../../../scripts/dev/smoke.ps1");
const PROCESS_SMOKE: &str = include_str!("../../../scripts/ci/service-process-smoke.sh");

#[test]
fn s09_compose_builds_real_services_and_local_policy_sidecars() {
    for service in ["web", "api", "realtime", "agent-worker", "admin"] {
        assert!(COMPOSE.contains(&format!("  {service}:")));
    }
    for command in [
        "api-server",
        "realtime-server",
        "agent-worker",
        "admin-server",
        "migration-runner",
    ] {
        assert!(COMPOSE.contains(&format!("command: [\"{command}\"]")));
        assert!(DOCKERFILE.contains(&format!("/usr/local/bin/{command}")));
    }
    assert!(DOCKERFILE.contains("FROM node:24-alpine AS web-builder"));
    assert!(DOCKERFILE.contains("FROM nginx:1.27-alpine AS web-runtime"));
    assert!(COMPOSE.contains("network_mode: \"service:openfga\""));
    assert!(COMPOSE.contains("TRPG_OPENFGA_STORE_ID_FILE"));
    assert!(COMPOSE.contains("TRPG_OPENFGA_MODEL_ID_FILE"));
    assert!(COMPOSE.contains("OPENFGA_DATASTORE_ENGINE: sqlite"));
    assert!(COMPOSE.contains("TRPG_OPA_POLICY_REVISION: opa-security-governance-v3"));
    assert!(CI_COMPOSE.contains("path: compose.yml"));
    assert!(!COMPOSE.contains("coc_ai_trpg.placeholder"));
    assert!(!COMPOSE.contains("not_implemented"));
    assert!(!CI_COMPOSE.contains("coc_ai_trpg.placeholder"));
    assert!(!CI_COMPOSE.contains("not_implemented"));
    assert!(DEV_SMOKE.contains("release_readiness.py"));
    assert!(DEV_SMOKE.contains("X-Smoke-Challenge"));
    assert!(DEV_SMOKE.contains("$response.placeholder -eq $true"));
    assert!(!DEV_SMOKE.contains("Result: PASS"));
}

#[test]
fn s09_release_process_smoke_uses_the_production_secret_boundary() {
    for required in [
        "TRPG_SECRET_MOUNT",
        "TRPG_SECRET_CATALOG_PATH",
        "TRPG_DATABASE_URL_SECRET_ID",
        "TRPG_WITNESS_DATABASE_URL_SECRET_ID",
        "TRPG_CANONICAL_HMAC_KEY_SECRET_ID",
        "TRPG_PAYLOAD_ENCRYPTION_KEY_SECRET_ID",
        "TRPG_IDENTITY_SIGNING_KEY_SECRET_ID",
        "TRPG_AUDIT_HMAC_KEY_SECRET_ID",
        "TRPG_REDIS_CACHE_KEY_ID",
        "TRPG_OBJECT_STORAGE_ACCESS_KEY_SECRET_ID",
        "TRPG_OBJECT_STORAGE_SECRET_KEY_SECRET_ID",
    ] {
        assert!(
            PROCESS_SMOKE.contains(required),
            "release process smoke omits {required}"
        );
    }
    for forbidden in [
        "TRPG_CANONICAL_HMAC_KEY_HEX",
        "TRPG_PAYLOAD_ENCRYPTION_KEY_HEX",
        "TRPG_IDENTITY_SIGNING_KEY_HEX",
        "TRPG_AUDIT_HMAC_KEY_HEX",
    ] {
        assert!(
            !PROCESS_SMOKE.contains(forbidden),
            "release process smoke still injects {forbidden}"
        );
    }
    assert!(PROCESS_SMOKE.contains("install -d -m 0700"));
    assert!(PROCESS_SMOKE.contains("umask 077"));
}

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
