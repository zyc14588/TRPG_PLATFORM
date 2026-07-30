use std::process::ExitCode;
use std::sync::Arc;

use admin_server::{AdminApplication, ProductionAdminOperations};
use trpg_contracts::{run_service_with_handler, RoleRuntimeProbe, ServiceKind, ServiceSpec};
use trpg_platform::admin_control_plane::AdminControlPlane;

fn main() -> ExitCode {
    let operations = match ProductionAdminOperations::from_environment() {
        Ok(operations) => Arc::new(operations),
        Err(code) => return startup_failure(code),
    };
    let control = match AdminControlPlane::from_environment(operations) {
        Ok(control) => control,
        Err(error) => return startup_failure(error.code()),
    };
    let application = AdminApplication::new(control);
    let readiness_application = application.clone();
    let runtime =
        match RoleRuntimeProbe::spawn("admin_runtime", move || readiness_application.readiness()) {
            Ok(runtime) => runtime,
            Err(error) => return startup_failure(error.code.as_str()),
        };
    let spec =
        match ServiceSpec::from_environment(ServiceKind::AdminServer, env!("CARGO_PKG_VERSION")) {
            Ok(spec) => spec,
            Err(error) => return startup_failure(error.code.as_str()),
        };
    match run_service_with_handler(
        spec,
        vec![runtime],
        Box::new(move |request| application.handle(request)),
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => startup_failure(error.code.as_str()),
    }
}

fn startup_failure(code: &str) -> ExitCode {
    eprintln!("service=admin-server error={code}");
    ExitCode::FAILURE
}
