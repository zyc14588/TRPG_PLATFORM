#![forbid(unsafe_code)]

use trpg_platform::deployment_ops::{
    validate_provider_boundary, DeploymentEnvironment, ProviderEndpoint,
};
use trpg_service_runtime::{serve, Check, Readiness, ServiceConfig};

fn main() {
    let unsafe_local_provider = ProviderEndpoint {
        provider: "local-runtime".to_owned(),
        api_key: String::new(),
        base_url: "http://0.0.0.0:11434".to_owned(),
        authenticated: false,
    };
    let unsafe_config_rejected =
        validate_provider_boundary(&DeploymentEnvironment::Production, &unsafe_local_provider)
            .is_err();
    let readiness = Readiness::new(vec![if unsafe_config_rejected {
        Check::pass(
            "admin_provider_boundary",
            "unsafe production provider configuration rejected",
        )
    } else {
        Check::fail(
            "admin_provider_boundary",
            "unsafe production provider configuration was accepted",
        )
    }]);

    if let Err(error) = serve(ServiceConfig {
        service: "admin-server",
        version: env!("CARGO_PKG_VERSION"),
        default_bind_addr: "127.0.0.1:8083",
        readiness,
    }) {
        eprintln!("admin-server failed: {error}");
        std::process::exit(1);
    }
}
