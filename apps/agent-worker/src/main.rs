use std::process::ExitCode;

use trpg_contracts::{run_service, RoleRuntimeProbe, ServiceKind, ServiceSpec};

fn main() -> ExitCode {
    run(
        ServiceKind::AgentWorker,
        RoleRuntimeProbe::spawn("agent_worker_runtime", || {
            let boundary = trpg_agent_runtime::provider_boundary_snapshot();
            if boundary.gateway == "Agent Gateway"
                && boundary.runtime == "Agent Orchestrator/Runtime"
                && boundary.provider_adapter == "Model Provider Adapter"
                && boundary.forbidden_direct_call_error == "DIRECT_LLM_CALL_FORBIDDEN"
            {
                Ok("gateway/runtime/provider adapter initialized".to_owned())
            } else {
                Err("provider boundary initialization is incomplete".to_owned())
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
