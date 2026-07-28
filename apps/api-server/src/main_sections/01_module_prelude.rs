use std::process::ExitCode;

use api_server::ApiApplication;
use trpg_contracts::{run_service_with_handler, RoleRuntimeProbe, ServiceKind, ServiceSpec};
use trpg_data_eventing::event_store_sqlx_outbox_projection::PostgresCanonicalStore;
use trpg_data_eventing::persistence_postgresql::CoreDomainRepository;
use trpg_identity::IdentityService;
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
    let application = if player_action_writes_enabled {
        let player_action_repository = match core_domain_repository_from_environment(
            &database_url,
            &canonical_runtime,
            canonical_store.clone(),
        ) {
            Ok(repository) => repository,
            Err(error) => {
                eprintln!("service=api-server error={error}");
                return ExitCode::FAILURE;
            }
        };
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

fn core_domain_repository_from_environment(
    database_url: &SecretValue,
    runtime: &tokio::runtime::Runtime,
    canonical_store: PostgresCanonicalStore,
) -> Result<CoreDomainRepository, String> {
    let mut connection = None;
    database_url
        .expose_utf8_to(|database| {
            connection =
                Some(runtime.block_on(CoreDomainRepository::connect(database, canonical_store)));
        })
        .map_err(|_| "DATABASE_URL_SECRET_INVALID".to_owned())?;
    connection
        .ok_or_else(|| "CORE_DOMAIN_DATABASE_CONNECTION_NOT_ATTEMPTED".to_owned())?
        .map_err(|_| "CORE_DOMAIN_DATABASE_CONNECTION_FAILED".to_owned())
}

fn deletion_repository_from_environment(
    database_url: &SecretValue,
) -> Result<(tokio::runtime::Runtime, PostgresDeletionRepository), String> {
    let runtime = tokio::runtime::Runtime::new()
        .map_err(|_| "PRIVACY_RUNTIME_INITIALIZATION_FAILED".to_owned())?;
    let mut connection = None;
    database_url
        .expose_utf8_to(|database| {
            connection = Some(runtime.block_on(PostgresDeletionRepository::connect(database)));
        })
        .map_err(|_| "DATABASE_URL_SECRET_INVALID".to_owned())?;
    let repository = connection
        .ok_or_else(|| "DELETION_DATABASE_CONNECTION_NOT_ATTEMPTED".to_owned())?
        .map_err(|_| "DELETION_DATABASE_CONNECTION_FAILED".to_owned())?;
    runtime
        .block_on(repository.check_readiness())
        .map_err(|_| "DELETION_SCHEMA_NOT_READY".to_owned())?;
    Ok((runtime, repository))
}

fn canonical_store_from_environment(
    secret_manager: &SecretManager<MountedFileSecretResolver>,
) -> Result<(tokio::runtime::Runtime, PostgresCanonicalStore), String> {
    let database_url = resolve_mounted_secret(secret_manager, "TRPG_CANONICAL_DATABASE_URL")?;
    let witness_database_url = resolve_mounted_secret(secret_manager, "TRPG_WITNESS_DATABASE_URL")?;
    let integrity_key_id = required_environment("TRPG_CANONICAL_HMAC_KEY_ID")?;
    let integrity_key = resolve_mounted_secret(secret_manager, "TRPG_CANONICAL_HMAC_KEY")?
        .to_key32()
        .map_err(|_| "CANONICAL_HMAC_KEY_INVALID".to_owned())?;
    let payload_key_id = required_environment("TRPG_PAYLOAD_ENCRYPTION_KEY_ID")?;
    let payload_key = resolve_mounted_secret(secret_manager, "TRPG_PAYLOAD_ENCRYPTION_KEY")?
        .to_key32()
        .map_err(|_| "PAYLOAD_ENCRYPTION_KEY_INVALID".to_owned())?;
    let runtime = tokio::runtime::Runtime::new()
        .map_err(|_| "CANONICAL_RUNTIME_INITIALIZATION_FAILED".to_owned())?;
    let mut connection = None;
    database_url
        .expose_utf8_to(|primary| {
            witness_database_url
                .expose_utf8_to(|witness| {
                    integrity_key.expose_to(|integrity| {
                        payload_key.expose_to(|payload| {
                            connection = Some(runtime.block_on(PostgresCanonicalStore::connect(
                                primary,
                                witness,
                                integrity_key_id,
                                integrity,
                                payload_key_id,
                                payload,
                            )));
                        });
                    });
                })
                .map_err(|_| "WITNESS_DATABASE_URL_SECRET_INVALID".to_owned())
        })
        .map_err(|_| "DATABASE_URL_SECRET_INVALID".to_owned())??;
    let store = connection
        .ok_or_else(|| "CANONICAL_STORE_CONNECTION_NOT_ATTEMPTED".to_owned())?
        .map_err(|error| format!("CANONICAL_STORE_CONNECTION_FAILED:{error}"))?;
    runtime
        .block_on(store.verify_integrity())
        .map_err(|error| format!("CANONICAL_STORE_NOT_READY:{error}"))?;
    Ok((runtime, store))
}

struct IdentitySecretConfiguration<'a> {
    database_url: &'a SecretValue,
    redis_url: &'a SecretValue,
    signing_key: &'a SecretKey32,
    postgres_ca: Option<&'a [u8]>,
    redis_ca: Option<&'a [u8]>,
    redis_client_certificate: Option<&'a [u8]>,
    redis_client_private_key: Option<&'a [u8]>,
    redis_namespace: &'a str,
    session_ttl_ms: u64,
    argon2_concurrency: usize,
}

fn identity_from_secrets(
    configuration: IdentitySecretConfiguration<'_>,
) -> Result<IdentityService, trpg_identity::IdentityError> {
    let mut identity = None;
    let database_result = configuration.database_url.expose_utf8_to(|database| {
        configuration.redis_url.expose_utf8_to(|redis| {
            configuration.signing_key.expose_to(|key| {
                identity = Some(
                    IdentityService::from_prepared_postgres_with_security_and_redis_tls(
                        database,
                        configuration.postgres_ca,
                        redis,
                        configuration.redis_namespace,
                        key,
                        configuration.session_ttl_ms,
                        configuration.argon2_concurrency,
                        configuration.redis_ca,
                        configuration.redis_client_certificate,
                        configuration.redis_client_private_key,
                    ),
                );
            });
        })
    });
    database_result
        .map_err(|_| trpg_identity::IdentityError::InvalidIdentityData)?
        .map_err(|_| trpg_identity::IdentityError::InvalidIdentityData)?;
    identity.unwrap_or(Err(trpg_identity::IdentityError::InvalidIdentityData))
}

fn optional_file_from_environment(name: &str) -> Result<Option<Vec<u8>>, String> {
    let Some(path) = std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
    else {
        return Ok(None);
    };
    std::fs::read(path)
        .map(Some)
        .map_err(|_| format!("{name}_UNREADABLE"))
}
