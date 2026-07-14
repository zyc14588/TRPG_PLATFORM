pub mod errors;
pub mod events;
pub mod health;
pub mod service;

pub use errors::WireErrorCode;
pub use events::{
    canonical_event_registry, canonical_event_schema, canonical_openapi_components,
    validate_event_registry, CanonicalEventHeader, EventContractError, EventDescriptor, EventType,
};
pub use health::{ComponentCheck, HealthState, ServiceKind, ServicePhase};
pub use service::{
    run_service, run_service_with_handler, HttpRequest, HttpResponse, RoleRuntimeProbe,
    ServiceError, ServiceRequestHandler, ServiceSpec,
};
