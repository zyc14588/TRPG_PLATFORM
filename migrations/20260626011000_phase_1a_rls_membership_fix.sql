CREATE OR REPLACE FUNCTION app.has_room_role(room_id uuid)
RETURNS boolean
LANGUAGE sql
STABLE
AS $$
    SELECT EXISTS (
        SELECT 1
        FROM public.room_members rm
        WHERE rm.room_id = $1
          AND rm.user_id = app.current_user_id()
          AND rm.role = app.current_room_role()
    )
$$;

DROP POLICY IF EXISTS room_invites_room_kp ON room_invites;
CREATE POLICY room_invites_room_kp ON room_invites
    USING (
        room_id = app.current_room_id()
        AND app.has_room_role(room_id)
        AND app.is_kp_role(app.current_room_role())
    )
    WITH CHECK (
        room_id = app.current_room_id()
        AND app.has_room_role(room_id)
        AND app.is_kp_role(app.current_room_role())
    );

DROP POLICY IF EXISTS rooms_room_context_select ON rooms;
CREATE POLICY rooms_room_context_select ON rooms
    FOR SELECT
    USING (
        id = app.current_room_id()
        AND (owner_id = app.current_user_id() OR app.has_room_role(id))
    );

DROP POLICY IF EXISTS room_members_context_select ON room_members;
CREATE POLICY room_members_context_select ON room_members
    FOR SELECT
    USING (
        room_id = app.current_room_id()
        AND user_id = app.current_user_id()
    );

DROP POLICY IF EXISTS room_members_kp_write ON room_members;
CREATE POLICY room_members_kp_write ON room_members
    FOR INSERT
    WITH CHECK (
        room_id = app.current_room_id()
        AND app.has_room_role(room_id)
        AND app.is_kp_role(app.current_room_role())
    );

DROP POLICY IF EXISTS sessions_context_select ON sessions;
CREATE POLICY sessions_context_select ON sessions
    FOR SELECT
    USING (room_id = app.current_room_id() AND app.has_room_role(room_id));

DROP POLICY IF EXISTS sessions_kp_write ON sessions;
CREATE POLICY sessions_kp_write ON sessions
    FOR ALL
    USING (
        room_id = app.current_room_id()
        AND app.has_room_role(room_id)
        AND app.is_kp_role(app.current_room_role())
    )
    WITH CHECK (
        room_id = app.current_room_id()
        AND app.has_room_role(room_id)
        AND app.is_kp_role(app.current_room_role())
    );

DROP POLICY IF EXISTS session_events_context_select ON session_events;
CREATE POLICY session_events_context_select ON session_events
    FOR SELECT
    USING (
        room_id = app.current_room_id()
        AND app.has_room_role(room_id)
        AND app.can_view_visibility(visibility_scope, actor_id)
    );

DROP POLICY IF EXISTS snapshots_context_select ON snapshots;
CREATE POLICY snapshots_context_select ON snapshots
    FOR SELECT
    USING (room_id = app.current_room_id() AND app.has_room_role(room_id));

DROP POLICY IF EXISTS characters_context_select ON characters;
CREATE POLICY characters_context_select ON characters
    FOR SELECT
    USING (
        room_id = app.current_room_id()
        AND app.has_room_role(room_id)
        AND app.can_view_visibility(visibility_scope, owner_id)
    );

DROP POLICY IF EXISTS characters_owner_or_kp_write ON characters;
CREATE POLICY characters_owner_or_kp_write ON characters
    FOR ALL
    USING (
        room_id = app.current_room_id()
        AND app.has_room_role(room_id)
        AND (owner_id = app.current_user_id() OR app.is_kp_role(app.current_room_role()))
    )
    WITH CHECK (
        room_id = app.current_room_id()
        AND app.has_room_role(room_id)
        AND (owner_id = app.current_user_id() OR app.is_kp_role(app.current_room_role()))
    );

DROP POLICY IF EXISTS combat_states_context_select ON combat_states;
CREATE POLICY combat_states_context_select ON combat_states
    FOR SELECT
    USING (room_id = app.current_room_id() AND app.has_room_role(room_id));

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
            AND app.has_room_role(room_id)
            AND app.can_view_visibility(visibility_scope, uploaded_by)
        )
    );

DROP POLICY IF EXISTS documents_kp_write ON documents;
CREATE POLICY documents_kp_write ON documents
    FOR ALL
    USING (
        room_id = app.current_room_id()
        AND app.has_room_role(room_id)
        AND app.is_kp_role(app.current_room_role())
    )
    WITH CHECK (
        room_id = app.current_room_id()
        AND app.has_room_role(room_id)
        AND app.is_kp_role(app.current_room_role())
    );

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
            AND app.has_room_role(room_id)
            AND app.can_view_visibility(visibility_scope, NULL)
        )
    );

DROP POLICY IF EXISTS chunks_kp_write ON chunks;
CREATE POLICY chunks_kp_write ON chunks
    FOR ALL
    USING (
        room_id = app.current_room_id()
        AND app.has_room_role(room_id)
        AND app.is_kp_role(app.current_room_role())
    )
    WITH CHECK (
        room_id = app.current_room_id()
        AND app.has_room_role(room_id)
        AND app.is_kp_role(app.current_room_role())
    );

DROP POLICY IF EXISTS agent_runs_context_select ON agent_runs;
CREATE POLICY agent_runs_context_select ON agent_runs
    FOR SELECT
    USING (
        room_id = app.current_room_id()
        AND app.has_room_role(room_id)
        AND app.can_view_visibility(visibility_scope, NULL)
    );

DROP POLICY IF EXISTS generated_media_context_select ON generated_media;
CREATE POLICY generated_media_context_select ON generated_media
    FOR SELECT
    USING (
        room_id = app.current_room_id()
        AND app.has_room_role(room_id)
        AND app.can_view_visibility(visibility_scope, requested_by)
    );

DROP POLICY IF EXISTS audit_logs_kp_select ON audit_logs;
CREATE POLICY audit_logs_kp_select ON audit_logs
    FOR SELECT
    USING (
        room_id = app.current_room_id()
        AND app.has_room_role(room_id)
        AND app.is_kp_role(app.current_room_role())
    );
