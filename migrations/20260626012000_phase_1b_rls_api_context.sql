CREATE OR REPLACE FUNCTION app.current_user_email()
RETURNS text
LANGUAGE sql
STABLE
AS $$
    SELECT nullif(current_setting('app.user_email', true), '')
$$;

CREATE OR REPLACE FUNCTION app.is_current_room_member(target_room_id uuid)
RETURNS boolean
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = public, app
AS $$
    SELECT EXISTS (
        SELECT 1
        FROM public.room_members rm
        WHERE rm.room_id = target_room_id
          AND rm.user_id = app.current_user_id()
    )
$$;

CREATE OR REPLACE FUNCTION app.has_accepted_invite(target_room_id uuid, target_user_id uuid)
RETURNS boolean
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = public, app
AS $$
    SELECT EXISTS (
        SELECT 1
        FROM public.room_invites ri
        WHERE ri.room_id = target_room_id
          AND ri.accepted_by = target_user_id
          AND ri.status = 'accepted'
    )
$$;

DROP POLICY IF EXISTS rooms_room_context_select ON rooms;
CREATE POLICY rooms_room_context_select ON rooms
    FOR SELECT
    USING (
        owner_id = app.current_user_id()
        OR app.is_current_room_member(id)
        OR EXISTS (
            SELECT 1
            FROM public.room_invites ri
            WHERE ri.room_id = rooms.id
              AND ri.invited_email = app.current_user_email()
              AND ri.status = 'pending'
        )
    );

DROP POLICY IF EXISTS room_members_context_select ON room_members;
CREATE POLICY room_members_context_select ON room_members
    FOR SELECT
    USING (
        user_id = app.current_user_id()
        OR (
            room_id = app.current_room_id()
            AND app.is_current_room_member(room_id)
            AND app.is_kp_role(app.current_room_role())
        )
    );

DROP POLICY IF EXISTS room_members_kp_write ON room_members;
CREATE POLICY room_members_kp_write ON room_members
    FOR INSERT
    WITH CHECK (
        (
            user_id = app.current_user_id()
            AND role = 'owner'
        )
        OR (
            room_id = app.current_room_id()
            AND app.is_current_room_member(room_id)
            AND app.is_kp_role(app.current_room_role())
        )
        OR (
            room_id = app.current_room_id()
            AND user_id = app.current_user_id()
            AND role = app.current_room_role()
            AND app.has_accepted_invite(room_id, user_id)
        )
    );

DROP POLICY IF EXISTS room_invites_room_kp ON room_invites;
CREATE POLICY room_invites_room_kp ON room_invites
    USING (
        room_id = app.current_room_id()
        AND app.is_current_room_member(room_id)
        AND app.is_kp_role(app.current_room_role())
    )
    WITH CHECK (
        room_id = app.current_room_id()
        AND app.is_current_room_member(room_id)
        AND app.is_kp_role(app.current_room_role())
    );

DROP POLICY IF EXISTS room_invites_invitee_select ON room_invites;
CREATE POLICY room_invites_invitee_select ON room_invites
    FOR SELECT
    USING (invited_email = app.current_user_email());

DROP POLICY IF EXISTS room_invites_invitee_accept_update ON room_invites;
CREATE POLICY room_invites_invitee_accept_update ON room_invites
    FOR UPDATE
    USING (invited_email = app.current_user_email())
    WITH CHECK (
        invited_email = app.current_user_email()
        AND accepted_by = app.current_user_id()
        AND status = 'accepted'
    );

DROP POLICY IF EXISTS audit_logs_insert_current_context ON audit_logs;
CREATE POLICY audit_logs_insert_current_context ON audit_logs
    FOR INSERT
    WITH CHECK (
        (actor_id IS NULL OR actor_id = app.current_user_id())
        AND (room_id IS NULL OR room_id = app.current_room_id() OR app.is_current_room_member(room_id))
    );
