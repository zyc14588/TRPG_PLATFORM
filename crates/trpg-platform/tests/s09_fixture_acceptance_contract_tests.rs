use std::{fs, path::Path, process::Command};

use trpg_platform::deployment_ops::{
    validate_provider_boundary, DeploymentEnvironment, ProviderEndpoint,
};

const COMPOSE: &str = include_str!("../../../compose.yml");
const CI_COMPOSE: &str = include_str!("../../../docker-compose.ci.yml");
const DEV_SMOKE: &str = include_str!("../../../scripts/dev/smoke.ps1");

#[test]
fn s09_placeholders_are_explicit_and_cannot_claim_ready() {
    for compose in [COMPOSE, CI_COMPOSE] {
        for service in ["web", "api", "realtime", "agent-worker", "admin"] {
            assert!(compose.contains(&format!("  {service}:")));
        }
        assert!(compose.contains("coc_ai_trpg.placeholder: \"true\""));
        assert!(compose.contains("placeholder"));
        assert!(compose.contains("not_implemented"));
        assert!(!compose.contains("\"status\":\"ok\""));
    }
    assert!(DEV_SMOKE.contains("release_readiness.py"));
    assert!(DEV_SMOKE.contains("X-Smoke-Challenge"));
    assert!(DEV_SMOKE.contains("$response.placeholder -eq $true"));
    assert!(!DEV_SMOKE.contains("Result: PASS"));
}

#[test]
fn s09_release_readiness_recognizes_runtime_and_fails_closed_without_deployment() {
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
        "missing product deployment must block release"
    );
    let payload = fs::read_to_string(&report).expect("readiness report generated for this run");
    fs::remove_file(report).expect("remove temporary readiness report");
    assert!(payload.contains("\"status\": \"BLOCKED\""));
    assert!(!payload.contains("NO_PRODUCT_BINARY"));
    assert!(!payload.contains("AUD-001"));
    assert!(payload.contains("AUD-002"));
    assert!(payload.contains("AUD-006"));
    assert!(payload.contains("PLACEHOLDER_SERVICE"));
}

#[test]
fn s09_prod_provider_security_boundary_is_executable() {
    let endpoint = ProviderEndpoint {
        provider: "ollama".to_owned(),
        base_url: "http://0.0.0.0:11434/v1".to_owned(),
        api_key: "ollama".to_owned(),
        authenticated: false,
    };
    assert!(validate_provider_boundary(&DeploymentEnvironment::Production, &endpoint).is_err());
}
