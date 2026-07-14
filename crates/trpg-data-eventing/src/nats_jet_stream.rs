crate::define_data_event_module!(
    NatsJetStreamCommand,
    NatsJetStreamOperation,
    append_nats_jet_stream_event,
    "nats_jet_stream",
    "NatsJetStreamOutboxPublished",
    "data_eventing.nats_jet_stream.event_schema",
    crate::DataEventOperation::OutboxPublish,
    ["event_outbox", "nats_jetstream_consumer"]
);

crate::define_data_event_artifacts!(
    NatsJetStreamService,
    NatsJetStreamRepository,
    NatsJetStreamEvent,
    NatsJetStreamError,
    EVENT_TYPE,
    EVENT_SCHEMA_NAME
);

pub fn published_subjects() -> &'static [&'static str] {
    crate::DATA_EVENT_NATS_SUBJECTS
}
