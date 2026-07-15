use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use trpg_contracts::{run_service, RoleRuntimeProbe, ServiceKind, ServiceSpec};
use trpg_data_eventing::cache_redis_impl::RedisProjectionCache;
use trpg_data_eventing::event_bus_nats_impl::JetStreamOutboxPublisher;

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
        let database_url = required_environment("TRPG_DATABASE_URL")?;
        let nats_url = required_environment("TRPG_NATS_URL")?;
        let redis_url = required_environment("TRPG_REDIS_URL")?;
        let nats_ca = optional_regular_file("TRPG_NATS_CA_CERT_PATH")?;
        let nats_credentials = optional_regular_file("TRPG_NATS_CREDENTIALS_PATH")?;
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|_| "REALTIME_RUNTIME_INITIALIZATION_FAILED".to_owned())?;
        let jetstream = runtime
            .block_on(JetStreamOutboxPublisher::connect_with_credentials(
                &database_url,
                &nats_url,
                "realtime-jetstream-reader",
                nats_ca.as_deref(),
                nats_credentials.as_deref(),
            ))
            .map_err(|_| "REALTIME_JETSTREAM_CONNECTION_FAILED".to_owned())?;
        runtime
            .block_on(jetstream.ensure_stream())
            .map_err(|_| "REALTIME_JETSTREAM_NOT_READY".to_owned())?;
        let cache = runtime
            .block_on(RedisProjectionCache::connect(
                &redis_url,
                "trpg:realtime:projection",
            ))
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
        self.runtime
            .block_on(self.cache.get("readiness"))
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
