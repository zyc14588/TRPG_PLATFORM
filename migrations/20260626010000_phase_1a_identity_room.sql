CREATE SCHEMA IF NOT EXISTS app;

CREATE OR REPLACE FUNCTION app.current_user_id()
RETURNS uuid
LANGUAGE sql
STABLE
AS $$
    SELECT nullif(current_setting('app.user_id', true), '')::uuid
$$;

CREATE OR REPLACE FUNCTION app.current_room_id()
RETURNS uuid
LANGUAGE sql
STABLE
AS $$
    SELECT nullif(current_setting('app.room_id', true), '')::uuid
$$;

CREATE OR REPLACE FUNCTION app.current_room_role()
RETURNS text
LANGUAGE sql
STABLE
AS $$
    SELECT nullif(current_setting('app.room_role', true), '')
$$;

CREATE OR REPLACE FUNCTION app.is_kp_role(role text)
RETURNS boolean
LANGUAGE sql
IMMUTABLE
AS $$
    SELECT role IN ('owner', 'kp', 'assistant_kp')
$$;

CREATE OR REPLACE FUNCTION app.is_room_member_role(role text)
RETURNS boolean
LANGUAGE sql
IMMUTABLE
AS $$
    SELECT role IN ('owner', 'kp', 'assistant_kp', 'pl', 'observer', 'public_screen')
$$;

CREATE OR REPLACE FUNCTION app.can_view_visibility(scope text, owner_id uuid)
RETURNS boolean
LANGUAGE sql
STABLE
AS $$
    SELECT CASE
        WHEN scope IN ('public_rule', 'room_rule', 'pl_visible_clue') THEN true
        WHEN scope = 'character_private' THEN app.is_kp_role(app.current_room_role()) OR owner_id = app.current_user_id()
        WHEN scope = 'session_log' THEN app.current_room_role() <> 'public_screen'
        WHEN scope IN ('kp_only_module', 'kp_secret', 'memory_private') THEN app.is_kp_role(app.current_room_role())
        ELSE false
    END
$$;

ALTER TABLE room_members
    ADD COLUMN IF NOT EXISTS invited_by uuid REFERENCES users(id),
    ADD COLUMN IF NOT EXISTS updated_at timestamptz NOT NULL DEFAULT now();

CREATE TABLE IF NOT EXISTS auth_identities (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider text NOT NULL CHECK (provider IN ('magic_link', 'oidc', 'development')),
    provider_subject text NOT NULL,
    email text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (provider, provider_subject)
);

CREATE TABLE IF NOT EXISTS magic_link_challenges (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    email text NOT NULL,
    token_hash text NOT NULL UNIQUE,
    expires_at timestamptz NOT NULL,
    consumed_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS refresh_sessions (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    session_family_id uuid NOT NULL,
    current_token_hash text NOT NULL UNIQUE,
    previous_token_hash text,
    status text NOT NULL CHECK (status IN ('active', 'revoked', 'expired')),
    expires_at timestamptz NOT NULL,
    rotated_at timestamptz,
    revoked_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS room_invites (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    room_id uuid NOT NULL REFERENCES rooms(id) ON DELETE CASCADE,
    invited_email text NOT NULL,
    invited_role text NOT NULL CHECK (invited_role IN ('kp', 'assistant_kp', 'pl', 'observer', 'public_screen')),
    token_hash text NOT NULL UNIQUE,
    status text NOT NULL CHECK (status IN ('pending', 'accepted', 'revoked', 'expired')),
    invited_by uuid NOT NULL REFERENCES users(id),
    accepted_by uuid REFERENCES users(id),
    expires_at timestamptz NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS idempotency_keys (
    scope text NOT NULL,
    key text NOT NULL,
    request_hash text NOT NULL,
    status text NOT NULL CHECK (status IN ('in_progress', 'completed', 'failed')),
    response_json jsonb,
    created_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL,
    PRIMARY KEY (scope, key)
);

ALTER TABLE audit_logs
    ADD COLUMN IF NOT EXISTS outcome text NOT NULL DEFAULT 'success' CHECK (outcome IN ('success', 'failure'));

CREATE INDEX IF NOT EXISTS auth_identities_user_id_idx ON auth_identities(user_id);
CREATE INDEX IF NOT EXISTS magic_link_challenges_email_idx ON magic_link_challenges(email);
CREATE INDEX IF NOT EXISTS refresh_sessions_user_status_idx ON refresh_sessions(user_id, status);
CREATE INDEX IF NOT EXISTS refresh_sessions_family_idx ON refresh_sessions(session_family_id);
CREATE INDEX IF NOT EXISTS room_invites_room_status_idx ON room_invites(room_id, status);
CREATE INDEX IF NOT EXISTS room_invites_email_idx ON room_invites(invited_email);
CREATE INDEX IF NOT EXISTS idempotency_keys_expires_at_idx ON idempotency_keys(expires_at);
CREATE INDEX IF NOT EXISTS audit_logs_outcome_idx ON audit_logs(outcome, created_at);

ALTER TABLE auth_identities ENABLE ROW LEVEL SECURITY;
ALTER TABLE magic_link_challenges ENABLE ROW LEVEL SECURITY;
ALTER TABLE refresh_sessions ENABLE ROW LEVEL SECURITY;
ALTER TABLE room_invites ENABLE ROW LEVEL SECURITY;
ALTER TABLE idempotency_keys ENABLE ROW LEVEL SECURITY;

ALTER TABLE auth_identities FORCE ROW LEVEL SECURITY;
ALTER TABLE magic_link_challenges FORCE ROW LEVEL SECURITY;
ALTER TABLE refresh_sessions FORCE ROW LEVEL SECURITY;
ALTER TABLE room_invites FORCE ROW LEVEL SECURITY;
ALTER TABLE idempotency_keys FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS auth_identities_self ON auth_identities;
CREATE POLICY auth_identities_self ON auth_identities
    USING (user_id = app.current_user_id())
    WITH CHECK (user_id = app.current_user_id());

DROP POLICY IF EXISTS magic_link_no_direct_access ON magic_link_challenges;
CREATE POLICY magic_link_no_direct_access ON magic_link_challenges
    USING (false)
    WITH CHECK (false);

DROP POLICY IF EXISTS refresh_sessions_self ON refresh_sessions;
CREATE POLICY refresh_sessions_self ON refresh_sessions
    USING (user_id = app.current_user_id())
    WITH CHECK (user_id = app.current_user_id());

DROP POLICY IF EXISTS room_invites_room_kp ON room_invites;
CREATE POLICY room_invites_room_kp ON room_invites
    USING (room_id = app.current_room_id() AND app.is_kp_role(app.current_room_role()))
    WITH CHECK (room_id = app.current_room_id() AND app.is_kp_role(app.current_room_role()));

DROP POLICY IF EXISTS idempotency_no_direct_access ON idempotency_keys;
CREATE POLICY idempotency_no_direct_access ON idempotency_keys
    USING (false)
    WITH CHECK (false);

DROP POLICY IF EXISTS rooms_room_context_select ON rooms;
CREATE POLICY rooms_room_context_select ON rooms
    FOR SELECT
    USING (
        id = app.current_room_id()
        AND (owner_id = app.current_user_id() OR app.is_room_member_role(app.current_room_role()))
    );

DROP POLICY IF EXISTS rooms_owner_insert ON rooms;
CREATE POLICY rooms_owner_insert ON rooms
    FOR INSERT
    WITH CHECK (owner_id = app.current_user_id());

DROP POLICY IF EXISTS rooms_owner_update ON rooms;
CREATE POLICY rooms_owner_update ON rooms
    FOR UPDATE
    USING (id = app.current_room_id() AND owner_id = app.current_user_id() AND app.current_room_role() = 'owner')
    WITH CHECK (id = app.current_room_id() AND owner_id = app.current_user_id() AND app.current_room_role() = 'owner');

DROP POLICY IF EXISTS room_members_context_select ON room_members;
CREATE POLICY room_members_context_select ON room_members
    FOR SELECT
    USING (
        room_id = app.current_room_id()
        AND (user_id = app.current_user_id() OR app.is_kp_role(app.current_room_role()))
    );

DROP POLICY IF EXISTS room_members_kp_write ON room_members;
CREATE POLICY room_members_kp_write ON room_members
    FOR INSERT
    WITH CHECK (room_id = app.current_room_id() AND app.is_kp_role(app.current_room_role()));

DROP POLICY IF EXISTS sessions_context_select ON sessions;
CREATE POLICY sessions_context_select ON sessions
    FOR SELECT
    USING (room_id = app.current_room_id() AND app.is_room_member_role(app.current_room_role()));

DROP POLICY IF EXISTS sessions_kp_write ON sessions;
CREATE POLICY sessions_kp_write ON sessions
    FOR ALL
    USING (room_id = app.current_room_id() AND app.is_kp_role(app.current_room_role()))
    WITH CHECK (room_id = app.current_room_id() AND app.is_kp_role(app.current_room_role()));

DROP POLICY IF EXISTS session_events_context_select ON session_events;
CREATE POLICY session_events_context_select ON session_events
    FOR SELECT
    USING (
        room_id = app.current_room_id()
        AND app.can_view_visibility(visibility_scope, actor_id)
    );

DROP POLICY IF EXISTS snapshots_context_select ON snapshots;
CREATE POLICY snapshots_context_select ON snapshots
    FOR SELECT
    USING (room_id = app.current_room_id() AND app.is_room_member_role(app.current_room_role()));

DROP POLICY IF EXISTS characters_context_select ON characters;
CREATE POLICY characters_context_select ON characters
    FOR SELECT
    USING (
        room_id = app.current_room_id()
        AND app.can_view_visibility(visibility_scope, owner_id)
    );

DROP POLICY IF EXISTS characters_owner_or_kp_write ON characters;
CREATE POLICY characters_owner_or_kp_write ON characters
    FOR ALL
    USING (
        room_id = app.current_room_id()
        AND (owner_id = app.current_user_id() OR app.is_kp_role(app.current_room_role()))
    )
    WITH CHECK (
        room_id = app.current_room_id()
        AND (owner_id = app.current_user_id() OR app.is_kp_role(app.current_room_role()))
    );

DROP POLICY IF EXISTS combat_states_context_select ON combat_states;
CREATE POLICY combat_states_context_select ON combat_states
    FOR SELECT
    USING (room_id = app.current_room_id() AND app.is_room_member_role(app.current_room_role()));

DROP POLICY IF EXISTS documents_visibility_select ON documents;
CREATE POLICY documents_visibility_select ON documents
    FOR SELECT
    USING (
        (
            room_id IS NULL
            AND visibility_scope = 'public_rule'
        )
        OR (
            room_id = app.current_room_id()
            AND app.can_view_visibility(visibility_scope, uploaded_by)
        )
    );

DROP POLICY IF EXISTS documents_kp_write ON documents;
CREATE POLICY documents_kp_write ON documents
    FOR ALL
    USING (room_id = app.current_room_id() AND app.is_kp_role(app.current_room_role()))
    WITH CHECK (room_id = app.current_room_id() AND app.is_kp_role(app.current_room_role()));

DROP POLICY IF EXISTS chunks_visibility_select ON chunks;
CREATE POLICY chunks_visibility_select ON chunks
    FOR SELECT
    USING (
        (
            room_id IS NULL
            AND visibility_scope = 'public_rule'
        )
        OR (
            room_id = app.current_room_id()
            AND app.can_view_visibility(visibility_scope, NULL)
        )
    );

DROP POLICY IF EXISTS chunks_kp_write ON chunks;
CREATE POLICY chunks_kp_write ON chunks
    FOR ALL
    USING (room_id = app.current_room_id() AND app.is_kp_role(app.current_room_role()))
    WITH CHECK (room_id = app.current_room_id() AND app.is_kp_role(app.current_room_role()));

DROP POLICY IF EXISTS agent_runs_context_select ON agent_runs;
CREATE POLICY agent_runs_context_select ON agent_runs
    FOR SELECT
    USING (
        room_id = app.current_room_id()
        AND app.can_view_visibility(visibility_scope, NULL)
    );

DROP POLICY IF EXISTS generated_media_context_select ON generated_media;
CREATE POLICY generated_media_context_select ON generated_media
    FOR SELECT
    USING (
        room_id = app.current_room_id()
        AND app.can_view_visibility(visibility_scope, requested_by)
    );

DROP POLICY IF EXISTS audit_logs_kp_select ON audit_logs;
CREATE POLICY audit_logs_kp_select ON audit_logs
    FOR SELECT
    USING (room_id = app.current_room_id() AND app.is_kp_role(app.current_room_role()));
