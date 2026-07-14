crate::define_data_event_module!(
    DomainEventSourcingProjectionCommand,
    DomainEventSourcingProjectionOperation,
    append_domain_event_sourcing_projection_event,
    "domain_event_sourcing_projection",
    "DomainEventSourcingProjectionRebuilt",
    "data_eventing.domain_event_sourcing_projection.event_schema",
    crate::DataEventOperation::ProjectionRebuild,
    ["event_store", "projection_checkpoint"]
);

crate::define_data_event_artifacts!(
    DomainEventSourcingProjectionService,
    DomainEventSourcingProjectionRepository,
    DomainEventSourcingProjectionEvent,
    DomainEventSourcingProjectionError,
    EVENT_TYPE,
    EVENT_SCHEMA_NAME
);

pub const PROJECTION_REBUILD_SOURCE: &str = crate::EVENT_STORE_TABLE;
