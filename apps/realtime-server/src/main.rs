use std::process::ExitCode;

use trpg_contracts::{run_service, RoleRuntimeProbe, ServiceKind, ServiceSpec};

fn main() -> ExitCode {
    run(
        ServiceKind::RealtimeServer,
        RoleRuntimeProbe::spawn("realtime_runtime", || {
            let adapter = trpg_api::contract_core::realtime_adapter_contract();
            if adapter.visibility_filtered
                && adapter.reconnect_supported
                && adapter.multi_room_supported
                && !adapter.nats_subjects.is_empty()
            {
                Ok(format!(
                    "adapter initialized; subjects={}",
                    adapter.nats_subjects.len()
                ))
            } else {
                Err("realtime adapter initialization is incomplete".to_owned())
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
