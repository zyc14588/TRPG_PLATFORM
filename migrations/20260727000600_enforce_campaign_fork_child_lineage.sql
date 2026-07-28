-- Each child campaign has exactly one immutable fork lineage. The
-- application also serializes fork creation on the child identifier, while
-- this constraint remains the final database-level invariant.

ALTER TABLE public.campaign_forks
    ADD CONSTRAINT campaign_forks_child_lineage_unique UNIQUE (child_campaign_id);
