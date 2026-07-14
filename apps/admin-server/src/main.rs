use std::process::ExitCode;

use trpg_contracts::{run_service, RoleRuntimeProbe, ServiceKind, ServiceSpec};

fn main() -> ExitCode {
    run(
        ServiceKind::AdminServer,
        RoleRuntimeProbe::spawn("admin_runtime", || {
            let invariants = trpg_platform::PLATFORM_INFRASTRUCTURE_INVARIANTS;
            if invariants.len() >= 5
                && invariants.contains(&"business_layer_must_not_call_llm_directly")
                && invariants.contains(&"formal_decisions_go_through_tool_rules_state_event_log")
            {
                Ok(format!(
                    "platform policy initialized; invariants={}",
                    invariants.len()
                ))
            } else {
                Err("required platform policy is missing".to_owned())
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
