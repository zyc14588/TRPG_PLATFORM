-- Prevent UPDATE from moving the canonical HUMAN_KP owner membership out of
-- the campaign. INSERT/UPDATE checks also keep active keeper membership aligned
-- with the destination campaign's immutable Authority Contract.

CREATE OR REPLACE FUNCTION enforce_membership_authority_consistency()
RETURNS trigger
LANGUAGE plpgsql
AS $$
DECLARE
    current_mode TEXT;
    current_owner TEXT;
BEGIN
    IF TG_OP IN ('UPDATE', 'DELETE') THEN
        SELECT authority_mode, authority_owner
          INTO current_mode, current_owner
          FROM authority_contracts
         WHERE campaign_id = OLD.campaign_id;

        IF FOUND AND current_mode = 'HUMAN_KP' AND OLD.user_id = current_owner THEN
            IF TG_OP = 'DELETE' THEN
                RAISE EXCEPTION 'canonical HUMAN_KP owner membership cannot be moved, revoked, deleted, or downgraded';
            END IF;
            IF NEW.campaign_id <> OLD.campaign_id
               OR NEW.user_id <> OLD.user_id
               OR NEW.revoked_at IS NOT NULL
               OR NEW.role <> 'HUMAN_KEEPER' THEN
                RAISE EXCEPTION 'canonical HUMAN_KP owner membership cannot be moved, revoked, deleted, or downgraded';
            END IF;
        END IF;
    END IF;

    IF TG_OP IN ('INSERT', 'UPDATE') THEN
        SELECT authority_mode, authority_owner
          INTO current_mode, current_owner
          FROM authority_contracts
         WHERE campaign_id = NEW.campaign_id;

        IF FOUND
           AND NEW.revoked_at IS NULL
           AND NEW.role = 'HUMAN_KEEPER'
           AND (current_mode <> 'HUMAN_KP' OR NEW.user_id <> current_owner) THEN
            RAISE EXCEPTION 'membership conflicts with canonical Authority Contract';
        END IF;
    END IF;

    IF TG_OP = 'DELETE' THEN
        RETURN OLD;
    END IF;
    RETURN NEW;
END;
$$;
