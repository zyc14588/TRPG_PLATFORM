-- Audit records must carry the same visibility and Fact Provenance context as
-- the command/policy decision they witness.

ALTER TABLE audit_log
    ADD COLUMN IF NOT EXISTS visibility_label TEXT NOT NULL DEFAULT 'system_only',
    ADD COLUMN IF NOT EXISTS visibility_subject TEXT NOT NULL DEFAULT 'not_applicable',
    ADD COLUMN IF NOT EXISTS provenance_kind TEXT NOT NULL DEFAULT 'system_fixture',
    ADD COLUMN IF NOT EXISTS provenance_reference TEXT NOT NULL DEFAULT 'legacy_audit_record',
    ADD COLUMN IF NOT EXISTS provenance_recorded_by TEXT NOT NULL DEFAULT 'migration';
