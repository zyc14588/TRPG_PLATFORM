use std::process::ExitCode;

use trpg_contracts::{run_service, RoleRuntimeProbe, ServiceKind, ServiceSpec};
use trpg_data_eventing::event_store_sqlx_outbox_projection::PostgresCanonicalStore;
use trpg_security_governance::secret::{
    MountedFileSecretResolver, SecretManager, SecretReference, SecretValue,
};

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
        let manager = production_secret_manager()?;
        let primary_url = resolve_mounted_secret(&manager, "TRPG_DATABASE_URL")?;
        let witness_url = resolve_mounted_secret(&manager, "TRPG_WITNESS_DATABASE_URL")?;
        let key_id = required_environment("TRPG_CANONICAL_HMAC_KEY_ID")?;
        let key = resolve_mounted_secret(&manager, "TRPG_CANONICAL_HMAC_KEY")?
            .to_key32()
            .map_err(|_| "CANONICAL_HMAC_KEY_INVALID".to_owned())?;
        let payload_key_id = required_environment("TRPG_PAYLOAD_ENCRYPTION_KEY_ID")?;
        let payload_key = resolve_mounted_secret(&manager, "TRPG_PAYLOAD_ENCRYPTION_KEY")?
            .to_key32()
            .map_err(|_| "PAYLOAD_ENCRYPTION_KEY_INVALID".to_owned())?;
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|_| "MIGRATION_RUNTIME_INITIALIZATION_FAILED".to_owned())?;
        let mut connection = None;
        expose_store_connection(
            &runtime,
            CanonicalSecretInputs {
                primary: &primary_url,
                witness: &witness_url,
                integrity: &key,
                payload: &payload_key,
                integrity_key_id: &key_id,
                payload_key_id: &payload_key_id,
            },
            &mut connection,
        )?;
        let store = connection
            .ok_or_else(|| "CANONICAL_STORE_CONNECTION_NOT_ATTEMPTED".to_owned())?
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

fn production_secret_manager() -> Result<SecretManager<MountedFileSecretResolver>, String> {
    let mount = required_environment("TRPG_SECRET_MOUNT")?;
    let resolver =
        MountedFileSecretResolver::new(mount).map_err(|_| "SECRET_MOUNT_INVALID".to_owned())?;
    SecretManager::new_durable(resolver, required_environment("TRPG_SECRET_CATALOG_PATH")?)
        .map_err(|_| "SECRET_CATALOG_INVALID".to_owned())
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

struct CanonicalSecretInputs<'a> {
    primary: &'a SecretValue,
    witness: &'a SecretValue,
    integrity: &'a trpg_security_governance::secret::SecretKey32,
    payload: &'a trpg_security_governance::secret::SecretKey32,
    integrity_key_id: &'a str,
    payload_key_id: &'a str,
}

fn expose_store_connection(
    runtime: &tokio::runtime::Runtime,
    inputs: CanonicalSecretInputs<'_>,
    output: &mut Option<
        Result<
            PostgresCanonicalStore,
            trpg_data_eventing::event_store_sqlx_outbox_projection::CanonicalStoreError,
        >,
    >,
) -> Result<(), String> {
    inputs
        .primary
        .expose_utf8_to(|primary_url| {
            inputs.witness.expose_utf8_to(|witness_url| {
                inputs.integrity.expose_to(|integrity_key| {
                    inputs.payload.expose_to(|payload_key| {
                        *output = Some(runtime.block_on(PostgresCanonicalStore::connect(
                            primary_url,
                            witness_url,
                            inputs.integrity_key_id,
                            integrity_key,
                            inputs.payload_key_id,
                            payload_key,
                        )));
                    });
                });
            })
        })
        .map_err(|_| "DATABASE_URL_SECRET_INVALID".to_owned())?
        .map_err(|_| "WITNESS_DATABASE_URL_SECRET_INVALID".to_owned())?;
    Ok(())
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
