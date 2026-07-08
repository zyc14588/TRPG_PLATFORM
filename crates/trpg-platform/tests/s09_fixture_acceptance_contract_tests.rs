use trpg_platform::deployment_ops::{
    validate_provider_boundary, DeploymentEnvironment, ProviderEndpoint,
};

const S09_STAGE_FIXTURE: &str =
    include_str!("../../../fixtures/stages/S09_stage_acceptance_fixture.v1.json.md");
const S09_DETAILED_FIXTURE: &str =
    include_str!("../../../fixtures/stages/detailed/S09_platform_infrastructure_deployment_expected.current.json.md");
const DEFAULT_COMPOSE: &str = include_str!("../../../compose.yml");
const DEFAULT_COMPOSE_OVERRIDE: &str = include_str!("../../../compose.override.yml");
const DOCKER_COMPOSE_CI: &str = include_str!("../../../docker-compose.ci.yml");
const DEV_SMOKE: &str = include_str!("../../../scripts/dev/smoke.ps1");
const COMPOSE_CONFIG_EVIDENCE: &str =
    include_str!("../../../evidence/stages/S09/docker-compose-config.txt");
const COMPOSE_SMOKE_EVIDENCE: &str =
    include_str!("../../../evidence/stages/S09/docker-compose-smoke.txt");
const HEALTH_CHECK_EVIDENCE: &str = include_str!("../../../evidence/stages/S09/health-checks.json");

#[test]
fn s09_detailed_fixture_is_bound_to_platform_stage_gate() {
    assert!(S09_STAGE_FIXTURE.contains("\"stage\": \"S09\""));
    assert!(
        S09_STAGE_FIXTURE.contains("stages/s09-platform-infrastructure-deployment/TEST_PLAN.md")
    );
    assert!(S09_STAGE_FIXTURE.contains("\"p1_findings_allowed\": 0"));

    for service in [
        "web",
        "api",
        "realtime",
        "agent-worker",
        "postgres",
        "redis",
        "nats",
        "minio",
        "reverse-proxy",
    ] {
        assert!(S09_DETAILED_FIXTURE.contains(service));
        assert!(DEFAULT_COMPOSE.contains(&format!("{service}:")));
        assert!(DOCKER_COMPOSE_CI.contains(&format!("{service}:")));
    }

    assert!(DEFAULT_COMPOSE_OVERRIDE.contains("services:"));

    for evidence in [
        "evidence/stages/S09/docker-compose-config.txt",
        "evidence/stages/S09/docker-compose-smoke.txt",
        "evidence/stages/S09/health-checks.json",
    ] {
        assert!(S09_DETAILED_FIXTURE.contains(evidence));
    }

    for criterion in [
        "compose_config_valid",
        "all_core_services_healthy",
        "init_wizard_completes",
        "prod_security_boundary_enforced",
    ] {
        assert!(S09_DETAILED_FIXTURE.contains(criterion));
    }

    assert!(S09_DETAILED_FIXTURE
        .contains("docker compose config && docker compose up -d && pwsh ./scripts/dev/smoke.ps1"));
    assert!(DEV_SMOKE.contains("http://localhost:8080/healthz"));
    assert!(!DEV_SMOKE.contains("NON_CODE_REASON"));
    assert!(!DEV_SMOKE.contains("NOT_RUN_NON_CODE_REASON"));
}

#[test]
fn s09_runtime_evidence_satisfies_detailed_stage_gate() {
    let detailed = json_fence(S09_DETAILED_FIXTURE);

    assert_eq!(json_string_value(detailed, "stage"), Some("S09"));

    let required_evidence = json_string_array(detailed, "required_evidence");
    assert_eq!(
        required_evidence,
        [
            "evidence/stages/S09/docker-compose-config.txt",
            "evidence/stages/S09/docker-compose-smoke.txt",
            "evidence/stages/S09/health-checks.json"
        ]
    );

    let pass_criteria = json_string_array(detailed, "pass_criteria");
    for criterion in [
        "compose_config_valid",
        "all_core_services_healthy",
        "init_wizard_completes",
        "prod_security_boundary_enforced",
    ] {
        assert!(pass_criteria.contains(&criterion));
    }

    assert!(COMPOSE_CONFIG_EVIDENCE.contains("Result: PASS"));
    assert!(!COMPOSE_CONFIG_EVIDENCE.contains("BLOCKED"));
    assert!(!COMPOSE_CONFIG_EVIDENCE.contains("not recognized"));
    assert!(COMPOSE_CONFIG_EVIDENCE.contains("services:"));

    assert!(COMPOSE_SMOKE_EVIDENCE.contains("Result: PASS"));
    assert!(!COMPOSE_SMOKE_EVIDENCE.contains("NON_CODE_REASON"));
    assert!(!COMPOSE_SMOKE_EVIDENCE.contains("NOT_RUN_NON_CODE_REASON"));
    assert!(!COMPOSE_SMOKE_EVIDENCE.contains("No executable admin init wizard endpoint"));
    assert!(COMPOSE_SMOKE_EVIDENCE.contains("healthz http://localhost:8080/healthz => 200"));
    for smoke_check in [
        "init_wizard_completes",
        "InitialAdminCreated",
        "ProviderConnectionTested",
        "RulesPackageInitialized",
        "DatabaseInitialized",
        "WebSocketChecked",
        "RagChecked",
        "DiceSelfTestPassed",
    ] {
        assert!(
            COMPOSE_SMOKE_EVIDENCE.contains(&format!("{smoke_check}: PASS")),
            "{smoke_check} must have real executable PASS evidence"
        );
    }
    for service in [
        "web",
        "api",
        "realtime",
        "agent-worker",
        "postgres",
        "redis",
        "nats",
        "minio",
        "reverse-proxy",
        "admin",
    ] {
        assert!(
            COMPOSE_SMOKE_EVIDENCE
                .lines()
                .any(|line| line.contains(&format!(" {service} ")) && line.contains("(healthy)")),
            "{service} must be healthy in docker compose smoke evidence"
        );
    }

    assert_eq!(
        json_string_value(HEALTH_CHECK_EVIDENCE, "status"),
        Some("healthy")
    );
    assert_eq!(
        json_string_value(HEALTH_CHECK_EVIDENCE, "service"),
        Some("api")
    );
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

fn json_fence(markdown: &str) -> &str {
    let start = markdown.find("```json").expect("fixture has json fence") + "```json".len();
    let rest = &markdown[start..];
    let end = rest.find("```").expect("fixture closes json fence");
    rest[..end].trim()
}

fn json_string_value<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let marker = format!("\"{key}\"");
    let after_key = json.split_once(&marker)?.1;
    let after_colon = after_key.split_once(':')?.1.trim_start();
    let after_open = after_colon.strip_prefix('"')?;
    let end = after_open.find('"')?;
    Some(&after_open[..end])
}

fn json_string_array<'a>(json: &'a str, key: &str) -> Vec<&'a str> {
    let marker = format!("\"{key}\"");
    let after_key = json
        .split_once(&marker)
        .unwrap_or_else(|| panic!("missing array key {key}"))
        .1;
    let after_open = after_key
        .split_once('[')
        .unwrap_or_else(|| panic!("missing array open for {key}"))
        .1;
    let array = after_open
        .split_once(']')
        .unwrap_or_else(|| panic!("missing array close for {key}"))
        .0;

    array
        .lines()
        .filter_map(|line| {
            let line = line.trim().trim_end_matches(',');
            let value = line.strip_prefix('"')?.strip_suffix('"')?;
            Some(value)
        })
        .collect()
}
