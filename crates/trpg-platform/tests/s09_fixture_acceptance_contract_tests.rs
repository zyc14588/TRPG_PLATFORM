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
const PRODUCTION_SECURITY_SMOKE: &str =
    include_str!("../../../scripts/ci/production-security-smoke.sh");

fn top_level_mapping_entry<'a>(document: &'a str, section: &str, entry: &str) -> &'a str {
    let section_marker = format!("{section}:");
    let entry_marker = format!("  {entry}:");
    let mut in_section = false;
    let mut entry_start = None;
    let lines = document.lines().collect::<Vec<_>>();
    for (index, line) in lines.iter().enumerate() {
        if line.is_empty() {
            continue;
        }
        if !line.starts_with(' ') {
            if in_section {
                break;
            }
            in_section = *line == section_marker;
            continue;
        }
        if in_section && *line == entry_marker {
            entry_start = Some(index + 1);
            break;
        }
    }
    let start = entry_start.unwrap_or_else(|| panic!("missing {section}.{entry}"));
    let end = lines[start..]
        .iter()
        .position(|line| !line.starts_with("    "))
        .map_or(lines.len(), |offset| start + offset);
    let start_offset: usize = lines[..start].iter().map(|line| line.len() + 1).sum();
    let end_offset: usize = lines[..end].iter().map(|line| line.len() + 1).sum();
    &document[start_offset..end_offset.min(document.len())]
}

fn shell_array_entries<'a>(script: &'a str, name: &str) -> Vec<&'a str> {
    let opening = format!("{name}=(");
    let mut in_array = false;
    let mut entries = Vec::new();
    for line in script.lines() {
        let trimmed = line.trim();
        if !in_array {
            in_array = trimmed == opening;
            continue;
        }
        if trimmed == ")" {
            return entries;
        }
        if !trimmed.is_empty() && !trimmed.starts_with('#') {
            entries.push(trimmed);
        }
    }
    panic!("missing or unterminated shell array {name}");
}

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
    assert!(DOCKERFILE.contains(
        "FROM node:24-alpine@sha256:a0b9bf06e4e6193cf7a0f58816cc935ff8c2a908f81e6f1a95432d679c54fbfd AS web-builder"
    ));
    assert!(DOCKERFILE.contains(
        "FROM nginx:1.27-alpine@sha256:65645c7bb6a0661892a8b03b89d0743208a18dd2f3f17a54ef4b76fb8e2f2a10 AS web-runtime"
    ));
    assert!(COMPOSE.contains("network_mode: \"service:openfga\""));
    assert!(COMPOSE.contains("TRPG_OPENFGA_STORE_ID_FILE"));
    assert!(COMPOSE.contains("TRPG_OPENFGA_MODEL_ID_FILE"));
    assert!(COMPOSE.contains("OPENFGA_DATASTORE_ENGINE: sqlite"));
    assert!(COMPOSE.contains("TRPG_OPA_POLICY_REVISION: opa-security-governance-v3"));
    assert_eq!(
        shell_array_entries(PRODUCTION_SECURITY_SMOKE, "compose_command"),
        [
            "docker compose",
            "--project-name \"$project_name\"",
            "-f \"$root/compose.yml\"",
            "-f \"$root/docker-compose.ci.yml\"",
        ]
    );
    for secret in [
        "nats_url",
        "nats_authorization",
        "postgres_tls_certificate",
        "redis_acl",
        "minio_tls_certificate",
    ] {
        let production = top_level_mapping_entry(COMPOSE, "secrets", secret);
        assert!(
            production
                .lines()
                .any(|line| line.trim() == "external: true"),
            "production secret {secret} must be external"
        );
        let ci = top_level_mapping_entry(CI_COMPOSE, "secrets", secret);
        assert!(
            ci.lines().any(|line| line.trim() == "external: false"),
            "CI override must explicitly localize {secret}"
        );
        assert!(
            ci.lines().any(|line| {
                line.trim()
                    == format!(
                        "file: ${{TRPG_COMPOSE_SECRET_DIRECTORY:?set TRPG_COMPOSE_SECRET_DIRECTORY}}/{secret}"
                    )
            }),
            "CI override uses an unexpected source for {secret}"
        );
    }
    for service in ["api", "realtime", "agent-worker", "migration-runner"] {
        let service_block = top_level_mapping_entry(COMPOSE, "services", service);
        assert!(
            service_block
                .lines()
                .any(|line| line.trim() == "- postgres_ca_certificate"),
            "database client {service} must mount the trust anchor referenced by its verify-full URL"
        );
    }
    let backend_network = top_level_mapping_entry(COMPOSE, "networks", "backend");
    assert!(
        backend_network
            .lines()
            .any(|line| line.trim() == "internal: true"),
        "the database backend network must remain internal"
    );
    for service in ["postgres", "postgres-witness"] {
        let service_block = top_level_mapping_entry(COMPOSE, "services", service);
        let network_lines = service_block
            .lines()
            .skip_while(|line| line.trim() != "networks:")
            .skip(1)
            .take_while(|line| line.starts_with("      - "))
            .map(str::trim)
            .collect::<Vec<_>>();
        assert_eq!(
            network_lines,
            ["- backend"],
            "{service} must be attached only to the internal backend network"
        );
        assert!(
            !service_block.lines().any(|line| line.trim() == "ports:"),
            "production Compose must not publish {service} ports"
        );
    }
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
