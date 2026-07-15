use std::env;
use std::fs;

use trpg_identity::IdentityService;

const KEY: [u8; 32] = [0x71; 32];

#[test]
fn remote_postgres_uses_verified_tls_and_rejects_an_untrusted_chain() {
    let Ok(database_url) = env::var("P02_TLS_DATABASE_URL") else {
        eprintln!("skipped: set P02_TLS_DATABASE_URL for the real PostgreSQL TLS gate");
        return;
    };
    let Ok(ca_path) = env::var("P02_TLS_CA_CERT_PATH") else {
        eprintln!("skipped: set P02_TLS_CA_CERT_PATH for the real PostgreSQL TLS gate");
        return;
    };
    let Ok(redis_url) = env::var("P02_REDIS_URL") else {
        eprintln!("skipped: set P02_REDIS_URL for the production identity gate");
        return;
    };
    let ca = fs::read(ca_path).unwrap();

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
