CREATE EXTENSION IF NOT EXISTS pgcrypto;
CREATE EXTENSION IF NOT EXISTS vector;

CREATE TABLE users (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    email text NOT NULL UNIQUE,
    display_name text NOT NULL,
    status text NOT NULL DEFAULT 'active',
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE rooms (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_id uuid NOT NULL REFERENCES users(id),
    title text NOT NULL,
    system_name text NOT NULL,
    privacy_mode text NOT NULL CHECK (privacy_mode IN ('standard', 'private_hybrid', 'local_only')),
    region_id text NOT NULL DEFAULT 'local-1',
    version bigint NOT NULL DEFAULT 0,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE room_members (
    room_id uuid NOT NULL REFERENCES rooms(id) ON DELETE CASCADE,
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role text NOT NULL CHECK (role IN ('owner', 'kp', 'assistant_kp', 'pl', 'observer', 'public_screen')),
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (room_id, user_id)
);

CREATE TABLE sessions (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    room_id uuid NOT NULL REFERENCES rooms(id) ON DELETE CASCADE,
    phase text NOT NULL DEFAULT 'lobby',
    status text NOT NULL DEFAULT 'open',
    version bigint NOT NULL DEFAULT 0,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE session_events (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    room_id uuid NOT NULL REFERENCES rooms(id) ON DELETE CASCADE,
    session_id uuid NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    server_seq bigint NOT NULL,
    event_type text NOT NULL,
    visibility_scope text NOT NULL,
    payload jsonb NOT NULL DEFAULT '{}'::jsonb,
    actor_id uuid REFERENCES users(id),
    request_id uuid,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (session_id, server_seq)
);

CREATE TABLE snapshots (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    room_id uuid NOT NULL REFERENCES rooms(id) ON DELETE CASCADE,
    session_id uuid NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    version bigint NOT NULL,
    state_json jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (session_id, version)
);

CREATE TABLE characters (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    room_id uuid NOT NULL REFERENCES rooms(id) ON DELETE CASCADE,
    owner_id uuid NOT NULL REFERENCES users(id),
    system_name text NOT NULL,
    visibility_scope text NOT NULL DEFAULT 'character_private',
    sheet_json jsonb NOT NULL DEFAULT '{}'::jsonb,
    version bigint NOT NULL DEFAULT 0,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE combat_states (
    session_id uuid PRIMARY KEY REFERENCES sessions(id) ON DELETE CASCADE,
    room_id uuid NOT NULL REFERENCES rooms(id) ON DELETE CASCADE,
    state_json jsonb NOT NULL DEFAULT '{}'::jsonb,
    version bigint NOT NULL DEFAULT 0,
    updated_by uuid REFERENCES users(id),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE documents (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    room_id uuid REFERENCES rooms(id) ON DELETE CASCADE,
    document_type text NOT NULL,
    title text NOT NULL,
    status text NOT NULL DEFAULT 'pending_review',
    visibility_scope text NOT NULL,
    license_name text,
    license_url text,
    source_url text,
    content_hash text NOT NULL,
    uploaded_by uuid REFERENCES users(id),
    created_at timestamptz NOT NULL DEFAULT now(),
    ingested_at timestamptz,
    UNIQUE (content_hash)
);

CREATE TABLE chunks (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id uuid NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    room_id uuid REFERENCES rooms(id) ON DELETE CASCADE,
    session_id uuid REFERENCES sessions(id) ON DELETE CASCADE,
    section_path jsonb NOT NULL DEFAULT '[]'::jsonb,
    page_start integer,
    page_end integer,
    visibility_scope text NOT NULL,
    content text NOT NULL DEFAULT '',
    embedding vector(1536),
    metadata jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE agent_runs (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    room_id uuid REFERENCES rooms(id) ON DELETE CASCADE,
    session_id uuid REFERENCES sessions(id) ON DELETE SET NULL,
    agent_name text NOT NULL,
    status text NOT NULL,
    provider text NOT NULL DEFAULT 'mock',
    model text NOT NULL DEFAULT 'mock',
    visibility_scope text NOT NULL DEFAULT 'system_internal',
    usage_json jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at timestamptz NOT NULL DEFAULT now(),
    finished_at timestamptz
);

CREATE TABLE outbox (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    topic text NOT NULL,
    payload jsonb NOT NULL DEFAULT '{}'::jsonb,
    status text NOT NULL DEFAULT 'pending',
    created_at timestamptz NOT NULL DEFAULT now(),
    processed_at timestamptz
);

CREATE TABLE generated_media (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    room_id uuid REFERENCES rooms(id) ON DELETE CASCADE,
    session_id uuid REFERENCES sessions(id) ON DELETE SET NULL,
    provider text NOT NULL DEFAULT 'mock',
    model text NOT NULL DEFAULT 'mock',
    status text NOT NULL CHECK (status IN ('draft', 'approved', 'published', 'rejected')),
    visibility_scope text NOT NULL,
    object_key text NOT NULL,
    license_json jsonb NOT NULL DEFAULT '{}'::jsonb,
    requested_by uuid REFERENCES users(id),
    approved_by uuid REFERENCES users(id),
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE audit_logs (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    room_id uuid REFERENCES rooms(id) ON DELETE CASCADE,
    actor_id uuid REFERENCES users(id),
    action text NOT NULL,
    target_type text NOT NULL,
    target_id uuid,
    scope text NOT NULL DEFAULT 'system_internal',
    payload_json jsonb NOT NULL DEFAULT '{}'::jsonb,
    request_id uuid,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX rooms_owner_id_idx ON rooms(owner_id);
CREATE INDEX room_members_user_id_idx ON room_members(user_id);
CREATE INDEX session_events_room_seq_idx ON session_events(room_id, server_seq);
CREATE INDEX documents_room_visibility_idx ON documents(room_id, visibility_scope, status);
CREATE INDEX chunks_document_id_idx ON chunks(document_id);
CREATE INDEX chunks_visibility_idx ON chunks(room_id, visibility_scope);
CREATE INDEX chunks_metadata_gin_idx ON chunks USING gin(metadata);
CREATE INDEX agent_runs_room_created_idx ON agent_runs(room_id, created_at);
CREATE INDEX audit_logs_room_created_idx ON audit_logs(room_id, created_at);

ALTER TABLE rooms ENABLE ROW LEVEL SECURITY;
ALTER TABLE room_members ENABLE ROW LEVEL SECURITY;
ALTER TABLE sessions ENABLE ROW LEVEL SECURITY;
ALTER TABLE session_events ENABLE ROW LEVEL SECURITY;
ALTER TABLE snapshots ENABLE ROW LEVEL SECURITY;
ALTER TABLE characters ENABLE ROW LEVEL SECURITY;
ALTER TABLE combat_states ENABLE ROW LEVEL SECURITY;
ALTER TABLE documents ENABLE ROW LEVEL SECURITY;
ALTER TABLE chunks ENABLE ROW LEVEL SECURITY;
ALTER TABLE agent_runs ENABLE ROW LEVEL SECURITY;
ALTER TABLE generated_media ENABLE ROW LEVEL SECURITY;
ALTER TABLE audit_logs ENABLE ROW LEVEL SECURITY;

ALTER TABLE rooms FORCE ROW LEVEL SECURITY;
ALTER TABLE room_members FORCE ROW LEVEL SECURITY;
ALTER TABLE sessions FORCE ROW LEVEL SECURITY;
ALTER TABLE session_events FORCE ROW LEVEL SECURITY;
ALTER TABLE snapshots FORCE ROW LEVEL SECURITY;
ALTER TABLE characters FORCE ROW LEVEL SECURITY;
ALTER TABLE combat_states FORCE ROW LEVEL SECURITY;
ALTER TABLE documents FORCE ROW LEVEL SECURITY;
ALTER TABLE chunks FORCE ROW LEVEL SECURITY;
ALTER TABLE agent_runs FORCE ROW LEVEL SECURITY;
ALTER TABLE generated_media FORCE ROW LEVEL SECURITY;
ALTER TABLE audit_logs FORCE ROW LEVEL SECURITY;
