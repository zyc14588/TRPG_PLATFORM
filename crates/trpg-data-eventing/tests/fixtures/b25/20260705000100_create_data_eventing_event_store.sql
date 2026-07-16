CREATE TABLE IF NOT EXISTS event_store (
    sequence BIGSERIAL PRIMARY KEY,
    event_type TEXT NOT NULL,
    command_id TEXT NOT NULL,
    idempotency_key TEXT NOT NULL UNIQUE,
    expected_version BIGINT NOT NULL,
    authority_mode TEXT NOT NULL,
    authority_contract_version BIGINT NOT NULL,
    visibility_label TEXT NOT NULL,
    fact_provenance_kind TEXT NOT NULL,
    fact_provenance_reference TEXT NOT NULL,
    fact_recorded_by TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    causation_id TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS event_outbox (
    outbox_id BIGSERIAL PRIMARY KEY,
    event_id BIGINT NOT NULL REFERENCES event_store(sequence),
    event_sequence BIGINT NOT NULL REFERENCES event_store(sequence),
    nats_subject TEXT NOT NULL,
    idempotency_key TEXT NOT NULL UNIQUE,
    visibility_label TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    causation_id TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    published_at TIMESTAMPTZ,
    retry_count INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS projection_checkpoint (
    projection_name TEXT PRIMARY KEY,
    stream_id TEXT NOT NULL,
    version BIGINT NOT NULL,
    last_event_sequence BIGINT NOT NULL,
    projection_hash TEXT NOT NULL,
    rebuilt_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
