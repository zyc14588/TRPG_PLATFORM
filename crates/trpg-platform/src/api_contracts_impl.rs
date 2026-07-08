use trpg_shared_kernel::{CommandEnvelope, EventEnvelope, EventStore, KernelResult, TrpgError};

pub const API_COMMAND_CONTRACT_REGISTERED_EVENT: &str =
    "platform.api_contracts_impl.command_contract_registered";
pub const API_CONTRACTS_IMPL_METRIC_MODULE: &str = "api_contracts_impl";
pub const API_CONTRACTS_IMPL_REQUIRED_METRICS: &[&str] = &[
    "trpg_command_total",
    "trpg_event_append_latency_ms",
    "trpg_policy_deny_total",
    "trpg_projection_lag_events",
    "trpg_visibility_redaction_total",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegisterApiCommandContract {
    pub contract_id: String,
    pub route: String,
    pub schema_version: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApiContractsEvent {
    ApiCommandContractRegistered {
        contract_id: String,
        route: String,
        schema_version: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApiContractsError {
    ContractIdRequired,
    InternalPlatformRouteRequired,
}

impl From<ApiContractsError> for TrpgError {
    fn from(error: ApiContractsError) -> Self {
        match error {
            ApiContractsError::ContractIdRequired => {
                TrpgError::InvalidConfiguration("api_contract_id_required")
            }
            ApiContractsError::InternalPlatformRouteRequired => {
                TrpgError::InvalidConfiguration("internal_platform_route_required")
            }
        }
    }
}

pub type ApiContractsEventEnvelope = EventEnvelope<ApiContractsEvent>;
pub type ApiContractsRepository = EventStore<ApiContractsEvent>;

pub struct ApiContractsService;

impl ApiContractsService {
    pub fn register_api_command_contract(
        repository: &mut ApiContractsRepository,
        command: &CommandEnvelope<RegisterApiCommandContract>,
    ) -> KernelResult<ApiContractsEventEnvelope> {
        if command.payload.contract_id.trim().is_empty() {
            return Err(ApiContractsError::ContractIdRequired.into());
        }
        if !command.payload.route.starts_with("/api/") {
            return Err(ApiContractsError::InternalPlatformRouteRequired.into());
        }

        repository.append(
            command,
            API_COMMAND_CONTRACT_REGISTERED_EVENT,
            ApiContractsEvent::ApiCommandContractRegistered {
                contract_id: command.payload.contract_id.clone(),
                route: command.payload.route.clone(),
                schema_version: command.payload.schema_version,
            },
        )
    }
}

pub fn register_api_command_contract(
    repository: &mut ApiContractsRepository,
    command: &CommandEnvelope<RegisterApiCommandContract>,
) -> KernelResult<ApiContractsEventEnvelope> {
    ApiContractsService::register_api_command_contract(repository, command)
}
