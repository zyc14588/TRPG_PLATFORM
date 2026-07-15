crate::define_data_event_module!(
    NatsJetstreamCommand,
    NatsJetstreamOperation,
    append_nats_jetstream_event,
    "adr_0004_nats_jetstream",
    "NatsJetstreamDecisionRecorded",
    "data_eventing.adr_0004_nats_jetstream.event_schema",
    crate::DataEventOperation::ArchitectureDecisionRecord,
    ["architecture_decision_index", "nats_jetstream_consumer"]
);
