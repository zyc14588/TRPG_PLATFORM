use std::env;

use trpg_identity::{GlobalRole, IdentityError, IdentityService};

const KEY: [u8; 32] = [0x6d; 32];

#[test]
fn redis_rate_limit_is_shared_across_identity_instances() {
    let Ok(database_url) = env::var("P02_DATABASE_URL") else {
        eprintln!("skipped: set P02_DATABASE_URL for the real identity integration gate");
        return;
    };
    let Ok(redis_url) = env::var("P02_REDIS_URL") else {
        eprintln!("skipped: set P02_REDIS_URL for the real distributed-login gate");
        return;
    };
    let suffix = std::process::id();
    let login = format!("distributed-limit-{suffix}@example.test");
    let namespace = format!("p02:identity:test:{suffix}");

    let mut first = IdentityService::from_postgres_with_distributed_login_security(
        &database_url,
        &redis_url,
        &namespace,
        &KEY,
        60_000,
        2,
    )
    .unwrap();
    first
        .create_user(
            format!("distributed_user_{suffix}"),
            &login,
            "correct horse battery staple",
            GlobalRole::User,
        )
        .unwrap();
    let mut second = IdentityService::from_postgres_with_distributed_login_security(
        &database_url,
        &redis_url,
        &namespace,
        &KEY,
        60_000,
        2,
    )
    .unwrap();

    for attempt in 0..5 {
        let service = if attempt % 2 == 0 {
            &mut first
        } else {
            &mut second
        };
        assert_eq!(
            service.login(&login, "incorrect password", 1_000 + attempt),
            Err(IdentityError::InvalidCredentials)
        );
    }
    assert_eq!(
        second.login(&login, "correct horse battery staple", 1_010),
        Err(IdentityError::LoginRateLimited)
    );
    first.check_readiness().unwrap();
    second.check_readiness().unwrap();
}
