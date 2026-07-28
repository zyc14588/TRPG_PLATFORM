
fn timestamp_from_unix_ms(
    value: u64,
    field: &'static str,
) -> Result<DateTime<Utc>, CoreDomainRepositoryError> {
    let value = i64::try_from(value).map_err(|_| CoreDomainRepositoryError::InvalidInput(field))?;
    Utc.timestamp_millis_opt(value)
        .single()
        .ok_or(CoreDomainRepositoryError::InvalidInput(field))
}

fn validated_object(source: &str, field: &'static str) -> Result<Value, CoreDomainRepositoryError> {
    let value: Value =
        serde_json::from_str(source).map_err(|_| CoreDomainRepositoryError::InvalidInput(field))?;
    if !value.is_object() {
        return Err(CoreDomainRepositoryError::InvalidInput(field));
    }
    Ok(value)
}

fn valid_sha256(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    })
}

fn token_digest_matches(supplied: &str, expected: &str) -> bool {
    type HmacSha256 = Hmac<Sha256>;
    const COMPARISON_KEY: &[u8] = b"p06-invite-token-digest-comparison-v1";
    let mut expected_mac =
        HmacSha256::new_from_slice(COMPARISON_KEY).expect("fixed HMAC key is valid");
    expected_mac.update(expected.as_bytes());
    let expected_tag = expected_mac.finalize().into_bytes();
    let mut supplied_mac =
        HmacSha256::new_from_slice(COMPARISON_KEY).expect("fixed HMAC key is valid");
    supplied_mac.update(supplied.as_bytes());
    supplied_mac.verify_slice(expected_tag.as_slice()).is_ok()
}

fn canonical_event_idempotency_matches(stored: &str, command_key: &str) -> bool {
    stored == format!("{command_key}:0000")
}

fn database_error(
    operation: &'static str,
) -> impl FnOnce(sqlx::Error) -> CoreDomainRepositoryError {
    move |_| CoreDomainRepositoryError::Database(operation)
}
