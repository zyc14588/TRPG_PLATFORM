
#[tokio::test]
async fn canonical_and_witness_endpoints_must_be_distinct() {
    let (primary_url, _) = database_urls();
    let result = PostgresCanonicalStore::connect(
        &primary_url,
        &primary_url,
        "p02-canonical-test-key",
        KEY,
        "p05-canonical-payload-key",
        PAYLOAD_KEY,
    )
    .await;
    assert!(matches!(
        result,
        Err(CanonicalStoreError::Configuration(
            "independent_witness_endpoint_required"
        ))
    ));
}

#[test]
#[should_panic(expected = "canonical primary and witness reset targets must be distinct")]
fn canonical_reset_rejects_identical_targets_before_connecting() {
    let database_url = "postgresql://local@127.0.0.1:25432/canonical_reset_probe";
    assert_distinct_database_targets(database_url, database_url);
}

#[test]
#[should_panic(expected = "canonical PostgreSQL URL must name an explicit non-empty database")]
fn canonical_reset_rejects_a_missing_database_name() {
    database_identity(&PgConnectOptions::new().host("127.0.0.1").port(25432));
}
