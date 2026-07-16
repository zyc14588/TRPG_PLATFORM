use std::process::ExitCode;

use trpg_contracts::{run_service, RoleRuntimeProbe, ServiceKind, ServiceSpec};
use trpg_data_eventing::event_store_sqlx_outbox_projection::PostgresCanonicalStore;

fn main() -> ExitCode {
    let runtime = match MigrationRuntime::from_environment() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("service=migration-runner error={error}");
            return ExitCode::FAILURE;
        }
    };
    run(
        ServiceKind::MigrationRunner,
        RoleRuntimeProbe::spawn("migration_runtime", move || runtime.check_readiness()),
    )
}

struct MigrationRuntime {
    runtime: tokio::runtime::Runtime,
    store: PostgresCanonicalStore,
}

impl MigrationRuntime {
    fn from_environment() -> Result<Self, String> {
        let primary_url = required_environment("TRPG_DATABASE_URL")?;
        let witness_url = required_environment("TRPG_WITNESS_DATABASE_URL")?;
        let key_id = required_environment("TRPG_CANONICAL_HMAC_KEY_ID")?;
        let key = required_environment("TRPG_CANONICAL_HMAC_KEY_HEX")
            .ok()
            .and_then(|value| decode_key(&value))
            .ok_or_else(|| "CANONICAL_HMAC_KEY_INVALID".to_owned())?;
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|_| "MIGRATION_RUNTIME_INITIALIZATION_FAILED".to_owned())?;
        let store = runtime
            .block_on(PostgresCanonicalStore::connect(
                &primary_url,
                &witness_url,
                key_id,
                &key,
            ))
            .map_err(|_| "CANONICAL_STORE_CONNECTION_FAILED".to_owned())?;
        runtime
            .block_on(store.prepare_for_service())
            .map_err(|error| format!("CANONICAL_MIGRATION_OR_RECOVERY_FAILED:{error}"))?;
        Ok(Self { runtime, store })
    }

    fn check_readiness(&self) -> Result<String, String> {
        self.runtime
            .block_on(self.store.verify_integrity())
            .map_err(|_| "canonical primary/witness integrity verification failed".to_owned())?;
        Ok(format!(
            "canonical primary and independent witness migrations applied; registry_statements={}",
            trpg_data_eventing::persistence_migrations::migrator()
                .iter()
                .filter(|migration| migration.migration_type.is_up_migration())
                .count()
        ))
    }
}

fn required_environment(name: &str) -> Result<String, String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{name}_REQUIRED"))
}

fn decode_key(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let mut key = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        key[index] = (hex_nibble(pair[0])? << 4) | hex_nibble(pair[1])?;
    }
    Some(key)
}

fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
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
