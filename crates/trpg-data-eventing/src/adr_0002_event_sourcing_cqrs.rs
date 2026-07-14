crate::define_data_event_module!(
    Adr0002EventSourcingCqrsCommand,
    Adr0002EventSourcingCqrsOperation,
    append_adr_0002_event_sourcing_cqrs_event,
    "adr_0002_event_sourcing_cqrs",
    "Adr0002EventSourcingCqrsDecisionRecorded",
    "data_eventing.adr_0002_event_sourcing_cqrs.event_schema",
    crate::DataEventOperation::ArchitectureDecisionRecord,
    ["event_store", "projection_checkpoint"]
);
