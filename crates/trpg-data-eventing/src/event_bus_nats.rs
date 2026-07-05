crate::define_data_event_module!(
    EventBusNatsCommand,
    EventBusNatsOperation,
    append_event_bus_nats_event,
    "CODEX-0059-06-DATA-EVENTING-8ceec1d689",
    "event_bus_nats",
    "EventBusNatsEventRecorded",
    "data_eventing.event_bus_nats.event_schema",
    crate::DataEventOperation::OutboxPublish,
    ["event_outbox", "nats_jetstream_consumer"]
);
