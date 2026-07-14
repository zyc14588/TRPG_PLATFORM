use std::collections::HashSet;
use std::process::ExitCode;

use trpg_contracts::{run_service, RoleRuntimeProbe, ServiceKind, ServiceSpec};

fn main() -> ExitCode {
    run(
        ServiceKind::MigrationRunner,
        RoleRuntimeProbe::spawn("migration_runtime", || {
            let statements = trpg_data_eventing::persistence_migrations::migration_statements();
            let mut names = HashSet::new();
            if !statements.is_empty()
                && statements
                    .iter()
                    .all(|(name, sql)| names.insert(*name) && sql.contains("CREATE TABLE"))
            {
                Ok(format!(
                    "migration registry initialized; statements={}",
                    statements.len()
                ))
            } else {
                Err("migration registry initialization failed".to_owned())
            }
        }),
    )
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
