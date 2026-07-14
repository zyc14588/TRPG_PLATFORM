crate::define_data_event_module!(
    EventBusNatsImplCommand,
    EventBusNatsImplOperation,
    append_event_bus_nats_impl_event,
    "event_bus_nats_impl",
    "EventBusNatsImplRecorded",
    "data_eventing.event_bus_nats_impl.event_schema",
    crate::DataEventOperation::OutboxPublish,
    [
        "event_outbox",
        "nats_jetstream_consumer",
        "dead_letter_queue"
    ]
);

crate::define_data_event_artifacts!(
    EventBusNatsImplService,
    EventBusNatsImplRepository,
    EventBusNatsImplEvent,
    EventBusNatsImplError,
    EVENT_TYPE,
    EVENT_SCHEMA_NAME
);

pub const OUTBOX_FLOW_STATES: &[&str] = &["pending", "published", "retrying", "dead_lettered"];
pub const PUBLISH_SOURCE: &str = crate::OUTBOX_TABLE;
