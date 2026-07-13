#![forbid(unsafe_code)]

use trpg_service_runtime::{serve, Check, Readiness, ServiceConfig};

fn main() {
    let boundary = trpg_runtime::runtime::runtime_boundary_snapshot();
    let required = [
        "idempotency_key",
        "expected_version",
        "authority_mode",
        "visibility",
        "fact_provenance",
        "correlation_id",
        "causation_id",
    ];
    let boundary_valid = boundary.canon_store == "Event Store"
        && boundary.formal_write_path.contains("Event Store")
        && required
            .iter()
            .all(|field| boundary.required_command_fields.contains(field));
    let readiness = Readiness::new(vec![if boundary_valid {
        Check::pass(
            "realtime_runtime_boundary",
            format!(
                "{} governed command fields loaded",
                boundary.required_command_fields.len()
            ),
        )
    } else {
        Check::fail(
            "realtime_runtime_boundary",
            "runtime boundary validation failed",
        )
    }]);

    if let Err(error) = serve(ServiceConfig {
        service: "realtime-server",
        version: env!("CARGO_PKG_VERSION"),
        default_bind_addr: "127.0.0.1:8081",
        readiness,
    }) {
        eprintln!("realtime-server failed: {error}");
        std::process::exit(1);
    }
}
