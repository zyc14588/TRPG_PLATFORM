-- Forward-only identity authorization extension for private campaign groups.
-- Group membership is durable, campaign-scoped and revocable; replay still
-- requires an active campaign membership on every decision.

CREATE TABLE IF NOT EXISTS campaign_groups (
    campaign_id TEXT NOT NULL,
    group_id TEXT NOT NULL,
    created_by TEXT NOT NULL REFERENCES users(user_id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (campaign_id, group_id),
    CHECK (length(trim(campaign_id)) > 0),
    CHECK (length(trim(group_id)) > 0)
);

CREATE TABLE IF NOT EXISTS campaign_group_memberships (
    campaign_id TEXT NOT NULL,
    group_id TEXT NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(user_id),
    granted_by TEXT NOT NULL REFERENCES users(user_id),
    granted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    revoked_at TIMESTAMPTZ,
    PRIMARY KEY (campaign_id, group_id, user_id),
    FOREIGN KEY (campaign_id, group_id)
        REFERENCES campaign_groups(campaign_id, group_id)
);

CREATE INDEX IF NOT EXISTS campaign_group_memberships_active_user_idx
    ON campaign_group_memberships(campaign_id, user_id, group_id)
    WHERE revoked_at IS NULL;

CREATE OR REPLACE FUNCTION enforce_campaign_group_identity()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'campaign group identity and membership history are retained; revoke membership instead';
    END IF;
    IF NEW.campaign_id <> OLD.campaign_id OR NEW.group_id <> OLD.group_id THEN
        RAISE EXCEPTION 'campaign group identity cannot move between campaigns or groups';
    END IF;
    RETURN NEW;
END;
$$;

CREATE OR REPLACE FUNCTION enforce_campaign_group_membership_identity()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'campaign group membership history is retained; revoke membership instead';
    END IF;
    IF NEW.campaign_id <> OLD.campaign_id
       OR NEW.group_id <> OLD.group_id
       OR NEW.user_id <> OLD.user_id THEN
        RAISE EXCEPTION 'campaign group membership cannot move between campaigns, groups, or users';
    END IF;
    RETURN NEW;
END;
$$;

CREATE OR REPLACE FUNCTION enforce_campaign_group_membership_scope()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    IF NEW.revoked_at IS NULL AND NOT EXISTS (
        SELECT 1
          FROM campaign_memberships
         WHERE campaign_id = NEW.campaign_id
           AND user_id = NEW.user_id
           AND role IN ('CAMPAIGN_OWNER', 'PLAYER')
           AND revoked_at IS NULL
    ) THEN
        RAISE EXCEPTION 'active campaign group membership requires an active investigator membership';
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS campaign_groups_identity_guard ON campaign_groups;
CREATE TRIGGER campaign_groups_identity_guard
BEFORE UPDATE OR DELETE ON campaign_groups
FOR EACH ROW EXECUTE FUNCTION enforce_campaign_group_identity();

DROP TRIGGER IF EXISTS campaign_group_memberships_identity_guard ON campaign_group_memberships;
CREATE TRIGGER campaign_group_memberships_identity_guard
BEFORE UPDATE OR DELETE ON campaign_group_memberships
FOR EACH ROW EXECUTE FUNCTION enforce_campaign_group_membership_identity();

DROP TRIGGER IF EXISTS campaign_group_memberships_scope_guard ON campaign_group_memberships;
CREATE TRIGGER campaign_group_memberships_scope_guard
BEFORE INSERT OR UPDATE ON campaign_group_memberships
FOR EACH ROW EXECUTE FUNCTION enforce_campaign_group_membership_scope();
