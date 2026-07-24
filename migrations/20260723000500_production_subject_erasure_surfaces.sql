CREATE TABLE IF NOT EXISTS privacy_erased_subjects (
    subject_id TEXT PRIMARY KEY,
    erasure_digest TEXT NOT NULL CHECK (erasure_digest ~ '^sha256:[0-9a-f]{64}$'),
    erased_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE OR REPLACE FUNCTION reject_erased_user_reactivation()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog, public
AS $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM public.privacy_erased_subjects
         WHERE subject_id = NEW.user_id
    ) AND (
        NEW.disabled_at IS NULL
        OR NEW.login_normalized !~ '^deleted_[0-9a-f]{64}$'
        OR NEW.password_hash !~ '^DELETED_ACCOUNT_NO_LOGIN_[0-9a-f]{64}$'
    ) THEN
        RAISE EXCEPTION 'erased user cannot be reactivated';
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS users_erasure_guard ON users;
CREATE TRIGGER users_erasure_guard
BEFORE UPDATE ON users
FOR EACH ROW EXECUTE FUNCTION reject_erased_user_reactivation();

CREATE OR REPLACE FUNCTION reject_erased_subject_session()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog, public
AS $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM public.privacy_erased_subjects
         WHERE subject_id = NEW.user_id
    ) THEN
        RAISE EXCEPTION 'access cannot be granted to an erased subject';
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS sessions_erasure_guard ON sessions;
CREATE TRIGGER sessions_erasure_guard
BEFORE INSERT OR UPDATE ON sessions
FOR EACH ROW EXECUTE FUNCTION reject_erased_subject_session();

CREATE OR REPLACE FUNCTION reject_erased_subject_membership()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog, public
AS $$
BEGIN
    IF NEW.revoked_at IS NULL AND EXISTS (
        SELECT 1 FROM public.privacy_erased_subjects
         WHERE subject_id = NEW.user_id
    ) THEN
        RAISE EXCEPTION 'access cannot be granted to an erased subject';
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS campaign_memberships_erasure_guard ON campaign_memberships;
CREATE TRIGGER campaign_memberships_erasure_guard
BEFORE INSERT OR UPDATE ON campaign_memberships
FOR EACH ROW EXECUTE FUNCTION reject_erased_subject_membership();

DROP TRIGGER IF EXISTS campaign_group_memberships_erasure_guard
    ON campaign_group_memberships;
CREATE TRIGGER campaign_group_memberships_erasure_guard
BEFORE INSERT OR UPDATE ON campaign_group_memberships
FOR EACH ROW EXECUTE FUNCTION reject_erased_subject_membership();

CREATE INDEX IF NOT EXISTS rag_snapshot_chunk_subject_idx
    ON rag_snapshot_chunk(visibility_subject, source_event_sequence)
    WHERE visibility_subject <> 'not_applicable';
