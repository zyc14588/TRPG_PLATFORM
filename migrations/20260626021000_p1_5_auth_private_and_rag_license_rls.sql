DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_app_private') THEN
        CREATE ROLE trpg_app_private NOLOGIN BYPASSRLS;
    ELSE
        ALTER ROLE trpg_app_private NOLOGIN BYPASSRLS;
    END IF;
END
$$;

GRANT USAGE ON SCHEMA public, app TO trpg_app_private;
GRANT SELECT, INSERT, UPDATE, DELETE
    ON auth_identities, magic_link_challenges, refresh_sessions, idempotency_keys
    TO trpg_app_private;
GRANT trpg_app_private TO CURRENT_USER;

CREATE OR REPLACE FUNCTION app.current_rag_access_path()
RETURNS text
LANGUAGE sql
STABLE
AS $$
    SELECT nullif(current_setting('app.rag_access_path', true), '')
$$;

DROP POLICY IF EXISTS document_sources_visibility_select ON document_sources;
DROP POLICY IF EXISTS document_sources_kp_write ON document_sources;
CREATE POLICY document_sources_visibility_select ON document_sources
    FOR SELECT
    USING (
        license_status = 'allowed'
        AND (
            (
                room_id IS NULL
                AND visibility_scope = 'public_rule'
            )
            OR (
                room_id = app.current_room_id()
                AND app.has_room_role(room_id)
                AND app.can_view_visibility(visibility_scope, created_by)
            )
        )
    );

DROP POLICY IF EXISTS document_sources_review_select ON document_sources;
CREATE POLICY document_sources_review_select ON document_sources
    FOR SELECT
    USING (
        app.current_rag_access_path() = 'license_review'
        AND license_status = 'pending_review'
        AND room_id = app.current_room_id()
        AND app.has_room_role(room_id)
        AND app.is_kp_role(app.current_room_role())
    );

CREATE POLICY document_sources_kp_insert ON document_sources
    FOR INSERT
    WITH CHECK (
        room_id = app.current_room_id()
        AND app.has_room_role(room_id)
        AND app.is_kp_role(app.current_room_role())
    );

CREATE POLICY document_sources_kp_update ON document_sources
    FOR UPDATE
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

CREATE POLICY document_sources_kp_delete ON document_sources
    FOR DELETE
    USING (
        room_id = app.current_room_id()
        AND app.has_room_role(room_id)
        AND app.is_kp_role(app.current_room_role())
    );

DROP POLICY IF EXISTS documents_visibility_select ON documents;
DROP POLICY IF EXISTS documents_kp_write ON documents;
CREATE POLICY documents_visibility_select ON documents
    FOR SELECT
    USING (
        license_status = 'allowed'
        AND (
            source_id IS NULL
            OR EXISTS (
                SELECT 1
                FROM document_sources ds
                WHERE ds.id = documents.source_id
                  AND ds.license_status = 'allowed'
            )
        )
        AND (
            (
                room_id IS NULL
                AND visibility_scope = 'public_rule'
            )
            OR (
                room_id = app.current_room_id()
                AND app.has_room_role(room_id)
                AND app.can_view_visibility(visibility_scope, uploaded_by)
            )
        )
    );

CREATE POLICY documents_kp_insert ON documents
    FOR INSERT
    WITH CHECK (
        room_id = app.current_room_id()
        AND app.has_room_role(room_id)
        AND app.is_kp_role(app.current_room_role())
    );

CREATE POLICY documents_kp_update ON documents
    FOR UPDATE
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

CREATE POLICY documents_kp_delete ON documents
    FOR DELETE
    USING (
        room_id = app.current_room_id()
        AND app.has_room_role(room_id)
        AND app.is_kp_role(app.current_room_role())
    );

DROP POLICY IF EXISTS chunks_visibility_select ON chunks;
DROP POLICY IF EXISTS chunks_kp_write ON chunks;
CREATE POLICY chunks_visibility_select ON chunks
    FOR SELECT
    USING (
        license_status = 'allowed'
        AND EXISTS (
            SELECT 1
            FROM documents d
            WHERE d.id = chunks.document_id
              AND d.license_status = 'allowed'
        )
        AND (
            (
                room_id IS NULL
                AND visibility_scope = 'public_rule'
            )
            OR (
                room_id = app.current_room_id()
                AND app.has_room_role(room_id)
                AND app.can_view_visibility(visibility_scope, NULL)
            )
        )
    );

CREATE POLICY chunks_kp_insert ON chunks
    FOR INSERT
    WITH CHECK (
        room_id = app.current_room_id()
        AND app.has_room_role(room_id)
        AND app.is_kp_role(app.current_room_role())
    );

CREATE POLICY chunks_kp_update ON chunks
    FOR UPDATE
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

CREATE POLICY chunks_kp_delete ON chunks
    FOR DELETE
    USING (
        room_id = app.current_room_id()
        AND app.has_room_role(room_id)
        AND app.is_kp_role(app.current_room_role())
    );
