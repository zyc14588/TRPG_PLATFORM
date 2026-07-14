pub const IDENTITY_AUTHORIZATION_MIGRATION_NAME: &str = "create_identity_authorization";

pub const IDENTITY_AUTHORIZATION_MIGRATION_SQL: &str =
    include_str!("../../../migrations/20260714000100_create_identity_authorization.up.sql");

pub const IDENTITY_AUTHORIZATION_TABLES: &[&str] = &[
    "users",
    "sessions",
    "campaign_memberships",
    "authority_contracts",
    "audit_log",
];

pub fn migration_statement() -> (&'static str, &'static str) {
    (
        IDENTITY_AUTHORIZATION_MIGRATION_NAME,
        IDENTITY_AUTHORIZATION_MIGRATION_SQL,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_schema_contains_only_the_p02_security_tables_and_guards() {
        for table in IDENTITY_AUTHORIZATION_TABLES {
            assert!(
                IDENTITY_AUTHORIZATION_MIGRATION_SQL
                    .contains(&format!("CREATE TABLE IF NOT EXISTS {table}")),
                "missing identity migration table: {table}"
            );
        }
        assert!(IDENTITY_AUTHORIZATION_MIGRATION_SQL.contains("token_hash BYTEA"));
        assert!(!IDENTITY_AUTHORIZATION_MIGRATION_SQL.contains("access_token"));
        assert!(IDENTITY_AUTHORIZATION_MIGRATION_SQL.contains("authority_contracts_immutable"));
        assert!(IDENTITY_AUTHORIZATION_MIGRATION_SQL.contains("audit_log_append_only"));
        assert!(IDENTITY_AUTHORIZATION_MIGRATION_SQL.contains("audit_log_chain_guard"));
        assert!(IDENTITY_AUTHORIZATION_MIGRATION_SQL.contains("audit_log_no_truncate"));
        assert!(IDENTITY_AUTHORIZATION_MIGRATION_SQL.contains("integrity_key_id"));
        assert!(IDENTITY_AUTHORIZATION_MIGRATION_SQL.contains("hmac-sha256"));
        assert!(IDENTITY_AUTHORIZATION_MIGRATION_SQL.contains("membership_authority_consistency"));
        assert!(IDENTITY_AUTHORIZATION_MIGRATION_SQL.contains("authentication_reference"));
        assert!(IDENTITY_AUTHORIZATION_MIGRATION_SQL.contains("openfga_policy_revision"));
        assert!(IDENTITY_AUTHORIZATION_MIGRATION_SQL.contains("opa_policy_revision"));
    }
}
