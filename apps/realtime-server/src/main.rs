use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use realtime_server::{ProductionRealtimeBackend, RealtimeApplication, RealtimeBackend};
use trpg_api::api_web_socket::RealtimeLimits;
use trpg_contracts::{ServiceKind, ServiceSpec};
use trpg_data_eventing::cache_redis_impl::RedisProjectionCache;
use trpg_data_eventing::event_bus_nats_impl::JetStreamOutboxPublisher;
use trpg_data_eventing::event_store_sqlx_outbox_projection::PostgresCanonicalStore;
use trpg_data_eventing::realtime_identity::PersistentRealtimeIdentity;
use trpg_security_governance::secret::{
    MountedFileSecretResolver, SecretManager, SecretReference, SecretValue,
};

type ProductionApplication = RealtimeApplication<ProductionRealtimeBackend>;

fn main() -> ExitCode {
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(_) => {
            eprintln!("service=realtime-server error=REALTIME_RUNTIME_INITIALIZATION_FAILED");
            return ExitCode::FAILURE;
        }
    };
    let (application, jetstream) = match production_application(&runtime) {
        Ok(composition) => composition,
        Err(error) => {
            eprintln!("service=realtime-server error={error}");
            return ExitCode::FAILURE;
        }
    };
    let spec =
        match ServiceSpec::from_environment(ServiceKind::RealtimeServer, env!("CARGO_PKG_VERSION"))
        {
            Ok(spec) => spec,
            Err(error) => {
                eprintln!("service=realtime-server error={}", error.code);
                return ExitCode::FAILURE;
            }
        };
    if runtime
        .block_on(application.backend().check_readiness())
        .is_err()
    {
        eprintln!("service=realtime-server error=REALTIME_DEPENDENCY_NOT_READY");
        return ExitCode::FAILURE;
    }
    let listener = match runtime.block_on(tokio::net::TcpListener::bind(spec.bind_address)) {
        Ok(listener) => listener,
        Err(_) => {
            eprintln!("service=realtime-server error=REALTIME_LISTENER_BIND_FAILED");
            return ExitCode::FAILURE;
        }
    };
    runtime.spawn(notification_loop(Arc::clone(&application), jetstream));
    eprintln!(
        "service={} state=ready listening={}",
        spec.kind.as_str(),
        spec.bind_address
    );
    let router = application.router();
    let shutdown_application = Arc::clone(&application);
    match runtime.block_on(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                shutdown_signal().await;
                shutdown_application.shutdown_connections();
            })
            .await
    }) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => {
            eprintln!("service=realtime-server error=REALTIME_SERVER_FAILED");
            ExitCode::FAILURE
        }
    }
}

fn production_application(
    runtime: &tokio::runtime::Runtime,
) -> Result<(Arc<ProductionApplication>, JetStreamOutboxPublisher), String> {
    let secret_mount = PathBuf::from(required_environment("TRPG_SECRET_MOUNT")?);
    let secret_manager = SecretManager::new_durable(
        MountedFileSecretResolver::new(&secret_mount)
            .map_err(|_| "SECRET_MOUNT_INVALID".to_owned())?,
        required_environment("TRPG_SECRET_CATALOG_PATH")?,
    )
    .map_err(|_| "SECRET_CATALOG_INVALID".to_owned())?;
    let database_url = resolve_mounted_secret(&secret_manager, "TRPG_DATABASE_URL")?;
    let witness_url = resolve_mounted_secret(&secret_manager, "TRPG_WITNESS_DATABASE_URL")?;
    let nats_url = resolve_mounted_secret(&secret_manager, "TRPG_NATS_URL")?;
    let redis_url = resolve_mounted_secret(&secret_manager, "TRPG_REDIS_URL")?;
    let redis_ca = optional_regular_file_bytes("TRPG_REDIS_CA_CERT_PATH")?;
    let redis_client_certificate = optional_regular_file_bytes("TRPG_REDIS_CLIENT_CERT_PATH")?;
    let redis_client_private_key = optional_regular_file_bytes("TRPG_REDIS_CLIENT_KEY_PATH")?;
    let nats_ca = optional_regular_file("TRPG_NATS_CA_CERT_PATH")?;
    let nats_client_certificate = optional_regular_file("TRPG_NATS_CLIENT_CERT_PATH")?;
    let nats_client_private_key = optional_regular_file("TRPG_NATS_CLIENT_KEY_PATH")?;
    let nats_credentials = optional_regular_file("TRPG_NATS_CREDENTIALS_PATH")?;
    let integrity_key_id = required_environment("TRPG_CANONICAL_HMAC_KEY_ID")?;
    let integrity_key = resolve_mounted_secret(&secret_manager, "TRPG_CANONICAL_HMAC_KEY")?
        .to_key32()
        .map_err(|_| "CANONICAL_HMAC_KEY_INVALID".to_owned())?;
    let payload_key_id = required_environment("TRPG_PAYLOAD_ENCRYPTION_KEY_ID")?;
    let payload_key = resolve_mounted_secret(&secret_manager, "TRPG_PAYLOAD_ENCRYPTION_KEY")?
        .to_key32()
        .map_err(|_| "PAYLOAD_ENCRYPTION_KEY_INVALID".to_owned())?;
    let canonical = database_url
        .expose_utf8_to(|database| {
            witness_url.expose_utf8_to(|witness| {
                integrity_key.expose_to(|integrity| {
                    payload_key.expose_to(|payload| {
                        runtime.block_on(PostgresCanonicalStore::connect(
                            database,
                            witness,
                            &integrity_key_id,
                            integrity,
                            &payload_key_id,
                            payload,
                        ))
                    })
                })
            })
        })
        .map_err(|_| "DATABASE_URL_SECRET_INVALID".to_owned())?
        .map_err(|_| "WITNESS_DATABASE_URL_SECRET_INVALID".to_owned())?
        .map_err(|_| "CANONICAL_STORE_CONNECTION_FAILED".to_owned())?;
    runtime
        .block_on(canonical.verify_integrity())
        .map_err(|_| "CANONICAL_STORE_NOT_READY".to_owned())?;

    let mut jetstream_result = None;
    nats_url
        .expose_utf8_to(|nats| {
            jetstream_result = Some(runtime.block_on(
                JetStreamOutboxPublisher::connect_with_credentials(
                    canonical.clone(),
                    nats,
                    "realtime-canonical-notifier",
                    nats_ca.as_deref(),
                    nats_client_certificate.as_deref(),
                    nats_client_private_key.as_deref(),
                    nats_credentials.as_deref(),
                ),
            ));
        })
        .map_err(|_| "NATS_URL_SECRET_INVALID".to_owned())?;
    let jetstream = jetstream_result
        .ok_or_else(|| "REALTIME_JETSTREAM_CONNECTION_NOT_ATTEMPTED".to_owned())?
        .map_err(|_| "REALTIME_JETSTREAM_CONNECTION_FAILED".to_owned())?;
    runtime
        .block_on(jetstream.ensure_stream())
        .map_err(|_| "REALTIME_JETSTREAM_NOT_READY".to_owned())?;

    let cache_secret_id = required_environment("TRPG_REDIS_CACHE_KEY_ID")?;
    let cache_secret_version = required_environment("TRPG_REDIS_CACHE_KEY_VERSION")?
        .parse::<u64>()
        .ok()
        .filter(|version| *version > 0)
        .ok_or_else(|| "REDIS_CACHE_KEY_VERSION_INVALID".to_owned())?;
    let cache_reference = SecretReference::mounted(&cache_secret_id, cache_secret_version)
        .map_err(|_| "REDIS_CACHE_KEY_REFERENCE_INVALID".to_owned())?;
    secret_manager
        .register(&cache_reference)
        .map_err(|_| "REDIS_CACHE_KEY_REGISTRATION_FAILED".to_owned())?;
    let cache_secret = secret_manager
        .resolve(&cache_reference)
        .map_err(|_| "REDIS_CACHE_KEY_RESOLUTION_FAILED".to_owned())?;
    let cache_key = cache_secret
        .to_key32()
        .map_err(|_| "REDIS_CACHE_KEY_INVALID".to_owned())?;
    let cache_key_reference = format!("{}-v{}", cache_secret_id, cache_secret_version);
    let mut cache_connection = None;
    redis_url
        .expose_utf8_to(|redis| {
            cache_key.expose_to(|key| {
                cache_connection = Some(runtime.block_on(RedisProjectionCache::connect_with_tls(
                    redis,
                    "trpg:realtime:projection",
                    &cache_key_reference,
                    key,
                    redis_ca.as_deref(),
                    redis_client_certificate.as_deref(),
                    redis_client_private_key.as_deref(),
                )));
            });
        })
        .map_err(|_| "REDIS_URL_SECRET_INVALID".to_owned())?;
    let cache = cache_connection
        .ok_or_else(|| "REALTIME_REDIS_CONNECTION_NOT_ATTEMPTED".to_owned())?
        .map_err(|_| "REALTIME_REDIS_CONNECTION_FAILED".to_owned())?;

    let identity = PersistentRealtimeIdentity::new(canonical.primary_pool());
    let tenant_id = std::env::var("TRPG_TENANT_ID")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "default".to_owned());
    let mut backend = None;
    integrity_key.expose_to(|key| {
        backend = Some(ProductionRealtimeBackend::new(
            tenant_id,
            identity,
            canonical,
            jetstream.clone(),
            cache,
            "realtime_resume_v1",
            key,
            Duration::from_secs(15 * 60),
        ));
    });
    let backend = backend
        .ok_or_else(|| "REALTIME_BACKEND_NOT_CONSTRUCTED".to_owned())?
        .map_err(|_| "REALTIME_BACKEND_CONFIGURATION_INVALID".to_owned())?;
    let limits = realtime_limits_from_environment()?;
    let application = RealtimeApplication::new(backend, limits)
        .map_err(|_| "REALTIME_APPLICATION_CONFIGURATION_INVALID".to_owned())?;
    Ok((application, jetstream))
}

async fn notification_loop(
    application: Arc<ProductionApplication>,
    jetstream: JetStreamOutboxPublisher,
) {
    loop {
        if let Ok(mut subscription) = jetstream.subscribe_canonical_notifications().await {
            while let Ok(true) = subscription.next().await {
                application.notify_canonical_change();
            }
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        if let Ok(mut terminate) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = terminate.recv() => {}
            }
            return;
        }
    }
    let _ = tokio::signal::ctrl_c().await;
}

fn realtime_limits_from_environment() -> Result<RealtimeLimits, String> {
    let mut limits = RealtimeLimits::default();
    limits.max_connections =
        optional_positive("TRPG_REALTIME_MAX_CONNECTIONS")?.unwrap_or(limits.max_connections);
    limits.max_message_bytes =
        optional_positive("TRPG_REALTIME_MAX_MESSAGE_BYTES")?.unwrap_or(limits.max_message_bytes);
    limits.max_pending_events =
        optional_positive("TRPG_REALTIME_MAX_PENDING_EVENTS")?.unwrap_or(limits.max_pending_events);
    limits.replay_page_size =
        optional_positive("TRPG_REALTIME_REPLAY_PAGE_SIZE")?.unwrap_or(limits.replay_page_size);
    limits
        .validate()
        .map_err(|error| error.to_owned())
        .map(|()| limits)
}

fn optional_positive<T>(name: &str) -> Result<Option<T>, String>
where
    T: std::str::FromStr + PartialOrd + Default,
{
    let Some(raw) = std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
    else {
        return Ok(None);
    };
    let value = raw.parse::<T>().map_err(|_| format!("{name}_INVALID"))?;
    if value <= T::default() {
        return Err(format!("{name}_INVALID"));
    }
    Ok(Some(value))
}

fn optional_regular_file(name: &str) -> Result<Option<PathBuf>, String> {
    let Some(value) = std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
    else {
        return Ok(None);
    };
    let path = PathBuf::from(value);
    validate_regular_file(&path)?;
    Ok(Some(path))
}

fn optional_regular_file_bytes(name: &str) -> Result<Option<Vec<u8>>, String> {
    optional_regular_file(name)?
        .map(|path| fs::read(path).map_err(|_| format!("{name}_UNREADABLE")))
        .transpose()
}

fn validate_regular_file(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("ABSOLUTE_CONFIGURATION_PATH_REQUIRED".to_owned());
    }
    let metadata =
        fs::symlink_metadata(path).map_err(|_| "REQUIRED_CONFIGURATION_FILE_MISSING".to_owned())?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err("REGULAR_CONFIGURATION_FILE_REQUIRED".to_owned());
    }
    Ok(())
}

fn required_environment(name: &str) -> Result<String, String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{name}_REQUIRED"))
}

fn resolve_mounted_secret(
    manager: &SecretManager<MountedFileSecretResolver>,
    prefix: &str,
) -> Result<SecretValue, String> {
    let id = required_environment(&format!("{prefix}_SECRET_ID"))?;
    let version = required_environment(&format!("{prefix}_SECRET_VERSION"))?
        .parse::<u64>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| format!("{prefix}_SECRET_VERSION_INVALID"))?;
    let reference = SecretReference::mounted(id, version)
        .map_err(|_| format!("{prefix}_SECRET_REFERENCE_INVALID"))?;
    manager
        .register(&reference)
        .map_err(|_| format!("{prefix}_SECRET_REGISTRATION_FAILED"))?;
    manager
        .resolve(&reference)
        .map_err(|_| format!("{prefix}_SECRET_RESOLUTION_FAILED"))
}
