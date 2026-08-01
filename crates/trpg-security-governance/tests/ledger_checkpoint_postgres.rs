use sqlx::postgres::PgPoolOptions;
use sqlx::Row;
use trpg_security_governance::secret::{
    LedgerCheckpoint, LedgerCheckpointStore, PostgresLedgerCheckpointStore,
    LEDGER_CHECKPOINT_GENESIS_HASH,
};

fn label(byte: char) -> String {
    format!("hmac-sha256:{}", byte.to_string().repeat(64))
}

#[tokio::test]
async fn witness_checkpoint_is_monotonic_append_only_and_role_scoped() {
    let database_url = std::env::var("AR03_WITNESS_DATABASE_URL")
        .expect("AR03_WITNESS_DATABASE_URL is required for the ledger checkpoint gate");
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await
        .unwrap();
    trpg_data_eventing::persistence_migrations::witness_migrator()
        .run(&pool)
        .await
        .unwrap();

    let store = PostgresLedgerCheckpointStore::connect(&database_url).unwrap();
    let ledger_id = format!("secret-catalog:sha256:{:064x}", std::process::id() as u64);
    let first = LedgerCheckpoint::new(
        1,
        LEDGER_CHECKPOINT_GENESIS_HASH,
        label('1'),
        "ar03-test-key",
        label('a'),
    )
    .unwrap();
    let second = LedgerCheckpoint::new(
        2,
        first.chain_head(),
        label('2'),
        "ar03-test-key",
        label('b'),
    )
    .unwrap();
    store.append(&ledger_id, &first).unwrap();
    store.append(&ledger_id, &second).unwrap();
    store.append(&ledger_id, &second).unwrap();
    assert_eq!(store.latest(&ledger_id).unwrap(), Some(second.clone()));

    let stale = LedgerCheckpoint::new(
        1,
        LEDGER_CHECKPOINT_GENESIS_HASH,
        label('3'),
        "ar03-test-key",
        label('c'),
    )
    .unwrap();
    assert!(store.append(&ledger_id, &stale).is_err());
    let gap = LedgerCheckpoint::new(
        4,
        second.chain_head(),
        label('4'),
        "ar03-test-key",
        label('d'),
    )
    .unwrap();
    assert!(store.append(&ledger_id, &gap).is_err());

    for statement in [
        "UPDATE security_ledger_checkpoints SET chain_head = chain_head WHERE ledger_id = $1",
        "DELETE FROM security_ledger_checkpoints WHERE ledger_id = $1",
    ] {
        assert!(sqlx::query(statement)
            .bind(&ledger_id)
            .execute(&pool)
            .await
            .is_err());
    }
    assert!(sqlx::query("TRUNCATE security_ledger_checkpoints")
        .execute(&pool)
        .await
        .is_err());

    sqlx::query(
        "REVOKE ALL PRIVILEGES ON ALL TABLES IN SCHEMA public \
         FROM trpg_witness_append_service, trpg_witness_read_service",
    )
    .execute(&pool)
    .await
    .unwrap();
    for role in ["trpg_witness_append_service", "trpg_witness_read_service"] {
        let mut connection = pool.acquire().await.unwrap();
        sqlx::query(&format!("SET ROLE {role}"))
            .execute(&mut *connection)
            .await
            .unwrap();
        let (role_domain, role_head) = if role.ends_with("append_service") {
            ("role-append", label('5'))
        } else {
            ("role-read", label('6'))
        };
        let role_ledger_id = format!("{role_domain}:sha256:{:064x}", std::process::id() as u64);
        sqlx::query("SELECT public.append_security_ledger_checkpoint($1, 1, $2, $3, $4, $5)")
            .bind(&role_ledger_id)
            .bind(LEDGER_CHECKPOINT_GENESIS_HASH)
            .bind(&role_head)
            .bind("ar03-role-test-key")
            .bind(label('e'))
            .execute(&mut *connection)
            .await
            .unwrap();
        let observed: String =
            sqlx::query("SELECT chain_head FROM public.latest_security_ledger_checkpoint($1)")
                .bind(&role_ledger_id)
                .fetch_one(&mut *connection)
                .await
                .unwrap()
                .get("chain_head");
        assert_eq!(observed, role_head);
        for statement in [
            "SELECT count(*) FROM security_ledger_checkpoints",
            "INSERT INTO security_ledger_checkpoints \
             (ledger_id, sequence, previous_chain_head, chain_head, \
              integrity_key_id, checkpoint_mac) VALUES \
             ('secret-catalog:sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', \
              1, 'hmac-sha256:0000000000000000000000000000000000000000000000000000000000000000', \
              'hmac-sha256:1111111111111111111111111111111111111111111111111111111111111111', \
              'denied', \
              'hmac-sha256:2222222222222222222222222222222222222222222222222222222222222222')",
            "UPDATE security_ledger_checkpoints SET chain_head = chain_head",
            "DELETE FROM security_ledger_checkpoints",
            "TRUNCATE security_ledger_checkpoints",
        ] {
            assert!(sqlx::query(statement)
                .execute(&mut *connection)
                .await
                .is_err());
        }
        sqlx::query("RESET ROLE")
            .execute(&mut *connection)
            .await
            .unwrap();
    }
}

#[test]
fn remote_checkpoint_database_requires_verify_full_tls() {
    assert!(PostgresLedgerCheckpointStore::connect(
        "postgres://ledger@example.test/witness?sslmode=require"
    )
    .is_err());
    assert!(PostgresLedgerCheckpointStore::connect(
        "postgres://ledger@example.test/witness?sslmode=verify-full"
    )
    .is_ok());
}
