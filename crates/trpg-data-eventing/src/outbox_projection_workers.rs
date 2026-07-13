crate::define_data_event_module!(
    OutboxProjectionWorkersCommand,
    OutboxProjectionWorkersOperation,
    append_outbox_projection_workers_event,
    "outbox_projection_workers",
    "OutboxProjectionWorkerRecorded",
    "data_eventing.outbox_projection_workers.event_schema",
    crate::DataEventOperation::OutboxPublish,
    ["event_outbox", "projection_worker_checkpoint"]
);
