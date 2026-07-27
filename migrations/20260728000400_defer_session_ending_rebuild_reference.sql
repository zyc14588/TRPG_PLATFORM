-- A fork-materialized Session can receive a native Ending before a later
-- projection rebuild. The append-only reservation must survive while the
-- rebuild transaction deletes and recreates that Session projection.

ALTER TABLE core_domain.session_ending_reservations
    DROP CONSTRAINT session_ending_reservations_session_id_fkey;

ALTER TABLE core_domain.session_ending_reservations
    ADD CONSTRAINT session_ending_reservations_session_id_fkey
    FOREIGN KEY (session_id)
    REFERENCES core_domain.sessions(session_id)
    DEFERRABLE INITIALLY DEFERRED;
