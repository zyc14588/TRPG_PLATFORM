use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use trpg_contracts::{run_service, RoleRuntimeProbe, ServiceKind, ServiceSpec};
use trpg_data_eventing::cache_redis_impl::RedisProjectionCache;
use trpg_data_eventing::event_bus_nats_impl::JetStreamOutboxPublisher;
use trpg_data_eventing::event_store_sqlx_outbox_projection::PostgresCanonicalStore;
use trpg_security_governance::secret::{
    MountedFileSecretResolver, SecretManager, SecretReference, SecretValue,
};

fn main() -> ExitCode {
    let runtime = match RealtimeRuntime::from_environment() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("service=realtime-server error={error}");
            return ExitCode::FAILURE;
        }
    };
    run(
        ServiceKind::RealtimeServer,
        RoleRuntimeProbe::spawn("realtime_runtime", move || runtime.check_readiness()),
    )
}

struct RealtimeRuntime {
    runtime: tokio::runtime::Runtime,
    jetstream: JetStreamOutboxPublisher,
    cache: RedisProjectionCache,
}

impl RealtimeRuntime {
    fn from_environment() -> Result<Self, String> {
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
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|_| "REALTIME_RUNTIME_INITIALIZATION_FAILED".to_owned())?;
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
                        "realtime-jetstream-reader",
                        nats_ca.as_deref(),
                        nats_client_certificate.as_deref(),
                        nats_client_private_key.as_deref(),
                        nats_credentials.as_deref(),
                    ),
                ));
            })
            .map_err(|_| "NATS_URL_SECRET_INVALID".to_owned())?;
        let jetstream_result = jetstream_result
            .ok_or_else(|| "REALTIME_JETSTREAM_CONNECTION_NOT_ATTEMPTED".to_owned())?;
        let jetstream =
            jetstream_result.map_err(|_| "REALTIME_JETSTREAM_CONNECTION_FAILED".to_owned())?;
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
        let cache_key_reference = format!("{}-v{}", cache_secret_id, cache_secret_version);
        let mut cache_connection = None;
        redis_url
            .expose_utf8_to(|redis| {
                cache_secret.expose_to(|cache_key| {
                    cache_connection =
                        Some(runtime.block_on(RedisProjectionCache::connect_with_tls(
                            redis,
                            "trpg:realtime:projection",
                            &cache_key_reference,
                            cache_key,
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
        Ok(Self {
            runtime,
            jetstream,
            cache,
        })
    }

    fn check_readiness(&self) -> Result<String, String> {
        let adapter = trpg_api::contract_core::realtime_adapter_contract();
        if !adapter.visibility_filtered
            || !adapter.reconnect_supported
            || !adapter.multi_room_supported
            || adapter.nats_subjects.is_empty()
        {
            return Err("realtime adapter initialization is incomplete".to_owned());
        }
        self.runtime
            .block_on(self.jetstream.check_readiness())
            .map_err(|_| "realtime JetStream unavailable".to_owned())?;
        let mut cache = self.cache.clone();
        self.runtime
            .block_on(cache.check_readiness())
            .map_err(|_| "realtime Redis projection unavailable".to_owned())?;
        Ok(format!(
            "authenticated replay transport dependencies ready; subjects={}",
            adapter.nats_subjects.len()
        ))
    }
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

fn run(
    kind: ServiceKind,
    runtime: Result<RoleRuntimeProbe, trpg_contracts::ServiceError>,
) -> ExitCode {
    let runtime = match runtime {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("service={} error={}", kind.as_str(), error.code);
            return ExitCode::FAILURE;
        }
    };
    let spec = match ServiceSpec::from_environment(kind, env!("CARGO_PKG_VERSION")) {
        Ok(spec) => spec,
        Err(error) => {
            eprintln!("service={} error={}", kind.as_str(), error.code);
            return ExitCode::FAILURE;
        }
    };
    match run_service(spec, vec![runtime]) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("service={} error={}", kind.as_str(), error.code);
            ExitCode::FAILURE
        }
    }
}
