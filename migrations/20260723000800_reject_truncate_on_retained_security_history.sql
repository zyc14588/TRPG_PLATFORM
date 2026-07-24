CREATE OR REPLACE FUNCTION reject_retained_security_history_truncate()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog, public
AS $$
BEGIN
    RAISE EXCEPTION 'retained security history is append-only; TRUNCATE is forbidden';
END;
$$;

DROP TRIGGER IF EXISTS campaign_groups_truncate_guard ON campaign_groups;
CREATE TRIGGER campaign_groups_truncate_guard
BEFORE TRUNCATE ON campaign_groups
FOR EACH STATEMENT EXECUTE FUNCTION reject_retained_security_history_truncate();

DROP TRIGGER IF EXISTS campaign_group_memberships_truncate_guard
    ON campaign_group_memberships;
CREATE TRIGGER campaign_group_memberships_truncate_guard
BEFORE TRUNCATE ON campaign_group_memberships
FOR EACH STATEMENT EXECUTE FUNCTION reject_retained_security_history_truncate();

DROP TRIGGER IF EXISTS cloud_egress_consents_truncate_guard ON cloud_egress_consents;
CREATE TRIGGER cloud_egress_consents_truncate_guard
BEFORE TRUNCATE ON cloud_egress_consents
FOR EACH STATEMENT EXECUTE FUNCTION reject_retained_security_history_truncate();

DROP TRIGGER IF EXISTS cloud_egress_route_snapshots_truncate_guard
    ON cloud_egress_route_snapshots;
CREATE TRIGGER cloud_egress_route_snapshots_truncate_guard
BEFORE TRUNCATE ON cloud_egress_route_snapshots
FOR EACH STATEMENT EXECUTE FUNCTION reject_retained_security_history_truncate();

DROP TRIGGER IF EXISTS cloud_egress_audit_truncate_guard ON cloud_egress_audit;
CREATE TRIGGER cloud_egress_audit_truncate_guard
BEFORE TRUNCATE ON cloud_egress_audit
FOR EACH STATEMENT EXECUTE FUNCTION reject_retained_security_history_truncate();

DROP TRIGGER IF EXISTS cloud_egress_notices_no_truncate ON cloud_egress_notices;
DROP TRIGGER IF EXISTS cloud_egress_notices_truncate_guard ON cloud_egress_notices;
CREATE TRIGGER cloud_egress_notices_truncate_guard
BEFORE TRUNCATE ON cloud_egress_notices
FOR EACH STATEMENT EXECUTE FUNCTION reject_retained_security_history_truncate();

REVOKE TRUNCATE ON campaign_groups, campaign_group_memberships,
    cloud_egress_consents, cloud_egress_route_snapshots, cloud_egress_audit,
    cloud_egress_notices
    FROM PUBLIC;
