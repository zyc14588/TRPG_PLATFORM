#![forbid(unsafe_code)]

use std::env;
use trpg_platform::deployment_ops::{
    validate_provider_boundary, DeploymentEnvironment, ProviderEndpoint,
};
use trpg_service_runtime::{serve, Check, Readiness, ServiceConfig};

type ProviderConfig = Result<(DeploymentEnvironment, ProviderEndpoint), String>;

fn provider_config_from_env() -> ProviderConfig {
    let environment = match env::var("TRPG_ENVIRONMENT")
        .unwrap_or_else(|_| "development".to_owned())
        .as_str()
    {
        "development" => DeploymentEnvironment::Development,
        "production" => DeploymentEnvironment::Production,
        _ => return Err("TRPG_ENVIRONMENT must be either development or production".to_owned()),
    };
    let authenticated = env::var("TRPG_PROVIDER_AUTHENTICATED")
        .unwrap_or_else(|_| "false".to_owned())
        .parse::<bool>()
        .map_err(|_| "TRPG_PROVIDER_AUTHENTICATED must be true or false".to_owned())?;
    Ok((
        environment,
        ProviderEndpoint {
            provider: env::var("TRPG_PROVIDER").unwrap_or_else(|_| "local-runtime".to_owned()),
            api_key: env::var("TRPG_PROVIDER_API_KEY").unwrap_or_default(),
            base_url: env::var("TRPG_PROVIDER_BASE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:11434".to_owned()),
            authenticated,
        },
    ))
}

fn build_readiness(config: ProviderConfig) -> Readiness {
    let check = match config {
        Ok((environment, endpoint)) => {
            if validate_provider_boundary(&environment, &endpoint).is_ok() {
                Check::pass(
                    "admin_provider_boundary",
                    format!("{} provider configuration validated", environment.as_str()),
                )
            } else {
                Check::fail(
                    "admin_provider_boundary",
                    "active provider configuration rejected",
                )
            }
        }
        Err(error) => Check::fail("admin_provider_boundary", error),
    };
    Readiness::new(vec![check])
}

fn main() {
    let readiness = build_readiness(provider_config_from_env());

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsafe_production_provider_is_not_ready() {
        let readiness = build_readiness(Ok((
            DeploymentEnvironment::Production,
            ProviderEndpoint {
                provider: "local-runtime".to_owned(),
                api_key: String::new(),
                base_url: "http://127.0.0.1:11434".to_owned(),
                authenticated: false,
            },
        )));
        assert!(!readiness.is_ready());
    }
}
