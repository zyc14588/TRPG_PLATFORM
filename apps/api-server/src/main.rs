#![forbid(unsafe_code)]

use trpg_service_runtime::{serve, Check, Readiness, ServiceConfig};

fn build_readiness(
    contracts: &[trpg_api::contract_core::ApiRealtimeContract],
    adapter_valid: bool,
) -> Readiness {
    let registry_valid = !contracts.is_empty()
        && contracts
            .iter()
            .all(|contract| trpg_api::contract_core::validate_api_contract(contract).is_ok());
    Readiness::new(vec![
        if registry_valid {
            Check::pass(
                "api_contract_registry",
                format!("{} contracts validated", contracts.len()),
            )
        } else {
            Check::fail("api_contract_registry", "contract validation failed")
        },
        if adapter_valid {
            Check::pass("api_adapter_boundaries", "adapter boundaries validated")
        } else {
            Check::fail("api_adapter_boundaries", "adapter validation failed")
        },
    ])
}

fn main() {
    let contracts = trpg_api::api_realtime_contracts();
    let readiness = build_readiness(
        &contracts,
        trpg_api::contract_core::validate_primary_adapter_boundaries().is_ok(),
    );

    if let Err(error) = serve(ServiceConfig {
        service: "api-server",
        version: env!("CARGO_PKG_VERSION"),
        default_bind_addr: "127.0.0.1:8080",
        readiness,
    }) {
        eprintln!("api-server failed: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_contract_registry_is_not_ready() {
        assert!(!build_readiness(&[], true).is_ready());
    }
}
