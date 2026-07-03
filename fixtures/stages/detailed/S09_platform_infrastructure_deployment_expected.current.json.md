# Detailed Stage Acceptance Fixture — v2.21

```json
{
  "stage": "S09",
  "purpose": "验证 Docker Compose、一键部署、健康检查、初始化向导和 provider 安全边界。",
  "inputs": {
    "compose_files": [
      "compose.yml",
      "compose.override.yml"
    ],
    "services": [
      "web",
      "api",
      "realtime",
      "agent-worker",
      "postgres",
      "redis",
      "nats",
      "minio",
      "reverse-proxy"
    ],
    "env_profile": "dev"
  },
  "actions": [
    {
      "id": "compose_config",
      "type": "docker_compose_config"
    },
    {
      "id": "compose_up",
      "type": "docker_compose_smoke"
    },
    {
      "id": "health_checks",
      "type": "http_health_probe"
    },
    {
      "id": "init_wizard",
      "type": "admin_init_flow"
    }
  ],
  "expected_events": [
    {
      "type": "DeploymentSmokePassed",
      "services": [
        "api",
        "realtime",
        "agent-worker"
      ]
    },
    {
      "type": "InitialAdminCreated"
    },
    {
      "type": "ProviderConnectionTested"
    }
  ],
  "expected_records": [
    {
      "record": "HealthCheckSnapshot",
      "required_fields": [
        "service",
        "status",
        "latency_ms",
        "checked_at"
      ]
    },
    {
      "record": "DeploymentEvidence",
      "required_fields": [
        "compose_config_hash",
        "env_template_hash",
        "service_status"
      ]
    }
  ],
  "expected_errors": [
    {
      "case": "prod_placeholder_api_key",
      "error": "PLACEHOLDER_API_KEY_FORBIDDEN_IN_PROD"
    },
    {
      "case": "public_unauthenticated_local_llm",
      "error": "LOCAL_PROVIDER_SECURITY_BOUNDARY_VIOLATION"
    }
  ],
  "failure_cases": [
    {
      "id": "missing_healthcheck",
      "expected_error": "SERVICE_HEALTHCHECK_REQUIRED"
    }
  ],
  "required_evidence": [
    "evidence/stages/S09/docker-compose-config.txt",
    "evidence/stages/S09/docker-compose-smoke.txt",
    "evidence/stages/S09/health-checks.json"
  ],
  "automation_target": "docker compose config && docker compose up -d && pwsh ./scripts/dev/smoke.ps1",
  "pass_criteria": [
    "compose_config_valid",
    "all_core_services_healthy",
    "init_wizard_completes",
    "prod_security_boundary_enforced"
  ]
}
```
