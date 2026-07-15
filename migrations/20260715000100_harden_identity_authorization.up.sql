-- P02 forward-only hardening. Existing Authority Contracts remain immutable;
-- this migration closes owner-membership revocation/deletion and session
-- rotation races without rewriting the original applied migration.

CREATE UNIQUE INDEX IF NOT EXISTS sessions_single_rotation_child_idx
    ON sessions(rotated_from_session_id)
    WHERE rotated_from_session_id IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS campaign_single_active_human_keeper_idx
    ON campaign_memberships(campaign_id)
    WHERE role = 'HUMAN_KEEPER' AND revoked_at IS NULL;

ALTER TABLE audit_log
    ADD COLUMN IF NOT EXISTS requested_role TEXT NOT NULL DEFAULT 'not_applicable';

CREATE OR REPLACE FUNCTION enforce_membership_authority_consistency()
RETURNS trigger
LANGUAGE plpgsql
AS $$
DECLARE
    affected_campaign_id TEXT;
    affected_user_id TEXT;
    current_mode TEXT;
    current_owner TEXT;
BEGIN
    affected_campaign_id := CASE WHEN TG_OP = 'DELETE' THEN OLD.campaign_id ELSE NEW.campaign_id END;
    affected_user_id := CASE WHEN TG_OP = 'DELETE' THEN OLD.user_id ELSE NEW.user_id END;

    SELECT authority_mode, authority_owner
      INTO current_mode, current_owner
      FROM authority_contracts
     WHERE campaign_id = affected_campaign_id;

    IF FOUND THEN
        IF current_mode = 'HUMAN_KP'
           AND affected_user_id = current_owner
           AND (
               TG_OP = 'DELETE'
               OR NEW.revoked_at IS NOT NULL
               OR NEW.role <> 'HUMAN_KEEPER'
           ) THEN
            RAISE EXCEPTION 'canonical HUMAN_KP owner membership cannot be revoked, deleted, or downgraded';
        END IF;

        IF TG_OP <> 'DELETE' AND NEW.revoked_at IS NULL THEN
            IF NEW.role = 'HUMAN_KEEPER'
               AND (current_mode <> 'HUMAN_KP' OR NEW.user_id <> current_owner) THEN
                RAISE EXCEPTION 'membership conflicts with canonical Authority Contract';
            END IF;
        END IF;
    END IF;

    IF TG_OP = 'DELETE' THEN
        RETURN OLD;
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS membership_authority_consistency ON campaign_memberships;
CREATE TRIGGER membership_authority_consistency
BEFORE INSERT OR UPDATE OR DELETE ON campaign_memberships
FOR EACH ROW EXECUTE FUNCTION enforce_membership_authority_consistency();
