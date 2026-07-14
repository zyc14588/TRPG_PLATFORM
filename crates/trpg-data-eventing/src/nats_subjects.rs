crate::define_data_event_module!(
    NatsSubjectsCommand,
    NatsSubjectsOperation,
    append_nats_subjects_event,
    "nats_subjects",
    "NatsSubjectsRegistered",
    "data_eventing.nats_subjects.event_schema",
    crate::DataEventOperation::SchemaRegister,
    ["event_store", "event_outbox", "nats_subject_registry"]
);

crate::define_data_event_artifacts!(
    NatsSubjectsService,
    NatsSubjectsRepository,
    NatsSubjectsEvent,
    NatsSubjectsError,
    EVENT_TYPE,
    EVENT_SCHEMA_NAME
);

pub const REQUIRED_SUBJECTS: &[&str] = &[
    crate::NATS_EVENTS_APPENDED,
    crate::NATS_PROJECTION_REBUILD_REQUESTED,
    "trpg.outbox.retry.requested",
    "trpg.outbox.dead_lettered",
];
