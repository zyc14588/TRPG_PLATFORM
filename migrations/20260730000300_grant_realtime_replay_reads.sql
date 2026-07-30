-- AR07: the realtime service authenticates sessions and filters canonical
-- replay with live membership/group state. Keep the role strictly read-only
-- and do not expose password hashes or normalized login names.
DO $realtime_replay_reads$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_realtime_service') THEN
        GRANT SELECT (user_id, disabled_at)
            ON public.users TO trpg_realtime_service;
        GRANT SELECT (
            session_id, user_id, token_hash, issued_at, expires_at, revoked_at
        ) ON public.sessions TO trpg_realtime_service;
        GRANT SELECT (campaign_id, user_id, role, revoked_at)
            ON public.campaign_memberships TO trpg_realtime_service;
        GRANT SELECT (campaign_id, group_id)
            ON public.campaign_groups TO trpg_realtime_service;
        GRANT SELECT (campaign_id, group_id, user_id, revoked_at)
            ON public.campaign_group_memberships TO trpg_realtime_service;
        GRANT SELECT (campaign_id, authority_mode, contract_version)
            ON public.authority_contracts TO trpg_realtime_service;
        GRANT SELECT (subject_id, key_reference, wrapped_key, destroyed_at)
            ON public.privacy_subject_keys TO trpg_realtime_service;
    END IF;
END;
$realtime_replay_reads$;
