use std::process::ExitCode;

use api_server::{AgentJobRouteConfiguration, ApiApplication};
use trpg_contracts::{run_service_with_handler, RoleRuntimeProbe, ServiceKind, ServiceSpec};
use trpg_data_eventing::event_store_sqlx_outbox_projection::PostgresCanonicalStore;
use trpg_data_eventing::persistence_postgresql::CoreDomainRepository;
use trpg_identity::IdentityService;
use trpg_runtime::durable_workflow::DurableWorkflowStore;
use trpg_security_governance::policy_adapter::{
    HttpPolicyEndpoint, OpenFgaOpaPolicyAdapter, PolicyBackend,
};
use trpg_security_governance::secret::{
    MountedFileSecretResolver, SecretKey32, SecretManager, SecretReference, SecretValue,
};
use trpg_security_governance::security_privacy::PostgresDeletionRepository;
use trpg_security_governance::tamper_evident_audit::FileAuditLog;

fn main() -> ExitCode {
    let secret_manager = match production_secret_manager() {
        Ok(manager) => manager,
        Err(error) => {
            eprintln!("service=api-server error={error}");
            return ExitCode::FAILURE;
        }
    };
    let database_url = match resolve_mounted_secret(&secret_manager, "TRPG_DATABASE_URL") {
        Ok(value) => value,
        Err(error) => {
            eprintln!("service=api-server error={error}");
            return ExitCode::FAILURE;
        }
    };
    let signing_key = match resolve_mounted_secret(&secret_manager, "TRPG_IDENTITY_SIGNING_KEY")
        .and_then(|value| {
            value
                .to_key32()
                .map_err(|_| "IDENTITY_SIGNING_KEY_INVALID".to_owned())
        }) {
        Ok(key) => key,
        Err(error) => {
            eprintln!("service=api-server error={error}");
            return ExitCode::FAILURE;
        }
    };
    let session_ttl_ms = std::env::var("TRPG_IDENTITY_SESSION_TTL_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(8 * 60 * 60 * 1_000);
    let redis_url = match resolve_mounted_secret(&secret_manager, "TRPG_REDIS_URL") {
        Ok(value) => value,
        Err(error) => {
            eprintln!("service=api-server error={error}");
            return ExitCode::FAILURE;
        }
    };
    let redis_namespace = std::env::var("TRPG_REDIS_LOGIN_NAMESPACE")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "trpg:identity".to_owned());
    let argon2_concurrency = std::env::var("TRPG_ARGON2_MAX_CONCURRENCY")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(2);
    let postgres_ca = match optional_file_from_environment("TRPG_POSTGRES_CA_CERT_PATH") {
        Ok(value) => value,
        Err(error) => {
            eprintln!("service=api-server error={error}");
            return ExitCode::FAILURE;
        }
    };
    let redis_ca = match optional_file_from_environment("TRPG_REDIS_CA_CERT_PATH") {
        Ok(value) => value,
        Err(error) => {
            eprintln!("service=api-server error={error}");
            return ExitCode::FAILURE;
        }
    };
    let redis_client_certificate =
        match optional_file_from_environment("TRPG_REDIS_CLIENT_CERT_PATH") {
            Ok(value) => value,
            Err(error) => {
                eprintln!("service=api-server error={error}");
                return ExitCode::FAILURE;
            }
        };
    let redis_client_private_key =
        match optional_file_from_environment("TRPG_REDIS_CLIENT_KEY_PATH") {
            Ok(value) => value,
            Err(error) => {
                eprintln!("service=api-server error={error}");
                return ExitCode::FAILURE;
            }
        };
    let identity = match identity_from_secrets(IdentitySecretConfiguration {
        database_url: &database_url,
        redis_url: &redis_url,
        signing_key: &signing_key,
        postgres_ca: postgres_ca.as_deref(),
        redis_ca: redis_ca.as_deref(),
        redis_client_certificate: redis_client_certificate.as_deref(),
        redis_client_private_key: redis_client_private_key.as_deref(),
        redis_namespace: &redis_namespace,
        session_ttl_ms,
        argon2_concurrency,
    }) {
        Ok(identity) => identity,
        Err(error) => {
            eprintln!("service=api-server error={}", error.code());
            return ExitCode::FAILURE;
        }
    };
    let (policy, audit) = match policy_and_audit_from_environment(&secret_manager) {
        Ok(configuration) => configuration,
        Err(error) => {
            eprintln!("service=api-server error={error}");
            return ExitCode::FAILURE;
        }
    };
    let (canonical_runtime, canonical_store) =
        match canonical_store_from_environment(&secret_manager) {
            Ok(configuration) => configuration,
            Err(error) => {
                eprintln!("service=api-server error={error}");
                return ExitCode::FAILURE;
            }
        };
    let (privacy_runtime, deletion_repository) =
        match deletion_repository_from_environment(&database_url) {
            Ok(configuration) => configuration,
            Err(error) => {
                eprintln!("service=api-server error={error}");
                return ExitCode::FAILURE;
            }
        };
    let player_action_writes_enabled = match player_action_writes_enabled(
        std::env::var("TRPG_PLAYER_ACTION_WRITES_ENABLED")
            .ok()
            .as_deref(),
    ) {
        Ok(enabled) => enabled,
        Err(error) => {
            eprintln!("service=api-server error={error}");
            return ExitCode::FAILURE;
        }
    };
    let player_action_repository = if player_action_writes_enabled {
        match core_domain_repository_from_environment(
            &database_url,
            &canonical_runtime,
            canonical_store.clone(),
        ) {
            Ok(repository) => Some(repository),
            Err(error) => {
                eprintln!("service=api-server error={error}");
                return ExitCode::FAILURE;
            }
        }
    } else {
        None
    };
    let agent_job_route = match optional_agent_job_route_from_environment() {
        Ok(route) => route,
        Err(error) => {
            eprintln!("service=api-server error={error}");
            return ExitCode::FAILURE;
        }
    };
    let application = if let Some(route) = agent_job_route {
        let workflow = match agent_workflow_from_environment(&database_url, &canonical_runtime) {
            Ok(workflow) => workflow,
            Err(error) => {
                eprintln!("service=api-server error={error}");
                return ExitCode::FAILURE;
            }
        };
        match ApiApplication::new_production_governed_with_agent_jobs(
            identity,
            policy,
            audit,
            canonical_runtime,
            canonical_store,
            privacy_runtime,
            deletion_repository,
            player_action_repository,
            workflow,
            route,
        ) {
            Ok(application) => application,
            Err(error) => {
                eprintln!("service=api-server error={error}");
                return ExitCode::FAILURE;
            }
        }
    } else if let Some(player_action_repository) = player_action_repository {
        ApiApplication::new_production_governed_with_player_actions(
            identity,
            policy,
            audit,
            canonical_runtime,
            canonical_store,
            privacy_runtime,
            deletion_repository,
            player_action_repository,
        )
    } else {
        ApiApplication::new_production_governed(
            identity,
            policy,
            audit,
            canonical_runtime,
            canonical_store,
            privacy_runtime,
            deletion_repository,
        )
    };
    let readiness_application = application.clone();
    run(
        ServiceKind::ApiServer,
        RoleRuntimeProbe::spawn("api_runtime", move || {
            trpg_api::contract_core::validate_primary_adapter_boundaries()
                .map_err(|error| error.code().to_owned())?;
            readiness_application.readiness()
        }),
        application,
    )
}
