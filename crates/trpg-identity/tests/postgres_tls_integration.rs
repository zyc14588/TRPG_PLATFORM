use std::env;
use std::fs;

use postgres::{Client, NoTls};
use trpg_identity::IdentityService;
use url::Url;

const KEY: [u8; 32] = [0x71; 32];

#[test]
fn remote_postgres_uses_verified_tls_and_rejects_an_untrusted_chain() {
    let database_url = env::var("P02_TLS_DATABASE_URL")
        .expect("P02_TLS_DATABASE_URL is required for the real PostgreSQL TLS gate");
    let ca_path = env::var("P02_TLS_CA_CERT_PATH")
        .expect("P02_TLS_CA_CERT_PATH is required to verify the PostgreSQL certificate chain");
    let redis_url = env::var("P02_REDIS_URL")
        .expect("P02_REDIS_URL is required for the production identity gate");
    let ca = fs::read(ca_path).unwrap();

    let mut plaintext_url = Url::parse(&database_url).unwrap();
    plaintext_url
        .query_pairs_mut()
        .clear()
        .append_pair("sslmode", "disable");
    assert!(
        Client::connect(plaintext_url.as_str(), NoTls).is_err(),
        "the TLS fixture must reject plaintext TCP connections at PostgreSQL"
    );

    assert!(IdentityService::from_postgres_with_security(
        &database_url,
        None,
        &redis_url,
        "p02:tls:untrusted",
        &KEY,
        60_000,
        2,
    )
    .is_err());

    let mut identity = IdentityService::from_postgres_with_security(
        &database_url,
        Some(&ca),
        &redis_url,
        "p02:tls:verified",
        &KEY,
        60_000,
        2,
    )
    .unwrap();
    assert!(identity.is_persistent());
    assert!(identity.is_distributed_login_protected());
    identity.check_readiness().unwrap();
}
