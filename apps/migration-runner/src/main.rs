#![forbid(unsafe_code)]

use std::collections::HashSet;
use trpg_service_runtime::{serve, Check, Readiness, ServiceConfig};

fn build_readiness(migrations: &[(&str, &str)], required_columns: &[&str]) -> Readiness {
    let unique_names = migrations
        .iter()
        .map(|(name, _)| *name)
        .collect::<HashSet<_>>()
        .len()
        == migrations.len();
    let nonempty = !migrations.is_empty()
        && migrations
            .iter()
            .all(|(name, statement)| !name.trim().is_empty() && !statement.trim().is_empty());
    let event_store_sql = migrations
        .iter()
        .find(|(name, _)| *name == "create_event_store")
        .map(|(_, statement)| *statement);
    let schema_complete = event_store_sql.is_some_and(|statement| {
        required_columns
            .iter()
            .all(|column| statement.contains(column))
    });
    let registry_valid = unique_names && nonempty && schema_complete;
    Readiness::new(vec![if registry_valid {
        Check::pass(
            "migration_registry",
            format!("{} migrations validated", migrations.len()),
        )
    } else {
        Check::fail("migration_registry", "migration registry validation failed")
    }])
}

fn main() {
    let migrations = trpg_data_eventing::persistence_migrations::migration_statements();
    let required_columns =
        trpg_data_eventing::sqlx_migrations_contract::required_event_store_columns();
    let readiness = build_readiness(migrations, required_columns);

    if let Err(error) = serve(ServiceConfig {
        service: "migration-runner",
        version: env!("CARGO_PKG_VERSION"),
        default_bind_addr: "127.0.0.1:8084",
        readiness,
    }) {
        eprintln!("migration-runner failed: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_migration_registry_is_not_ready() {
        assert!(!build_readiness(&[], &["sequence"]).is_ready());
    }
}
