
impl IdentityService {

    fn verify_signed_credential<'a>(&self, credential: &'a str) -> Result<&'a str, IdentityError> {
        let (claims, supplied_signature) = credential
            .rsplit_once('.')
            .ok_or(IdentityError::InvalidInternalCredential)?;
        let supplied_signature = hex_decode_32(supplied_signature)?;
        let mut mac = HmacSha256::new_from_slice(&self.signing_key)
            .map_err(|_| IdentityError::InvalidSigningKey)?;
        mac.update(claims.as_bytes());
        mac.verify_slice(&supplied_signature)
            .map_err(|_| IdentityError::InvalidInternalCredential)?;
        Ok(claims)
    }
}

fn normalize_login(login: &str) -> Result<String, IdentityError> {
    let normalized = login.trim().to_ascii_lowercase();
    if normalized.len() < 3
        || normalized.len() > 254
        || !normalized
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "@._-+".contains(character))
    {
        return Err(IdentityError::InvalidIdentityData);
    }
    Ok(normalized)
}

fn global_role_name(role: GlobalRole) -> &'static str {
    match role {
        GlobalRole::User => "USER",
        GlobalRole::Moderator => "MODERATOR",
        GlobalRole::ServerOwner => "SERVER_OWNER",
    }
}

fn parse_global_role(value: &str) -> Result<GlobalRole, IdentityError> {
    match value {
        "USER" => Ok(GlobalRole::User),
        "MODERATOR" => Ok(GlobalRole::Moderator),
        "SERVER_OWNER" => Ok(GlobalRole::ServerOwner),
        _ => Err(IdentityError::InvalidIdentityData),
    }
}

fn campaign_role_name(role: CampaignRole) -> &'static str {
    match role {
        CampaignRole::CampaignOwner => "CAMPAIGN_OWNER",
        CampaignRole::HumanKeeper => "HUMAN_KEEPER",
        CampaignRole::Player => "PLAYER",
        CampaignRole::Spectator => "SPECTATOR",
    }
}

fn parse_campaign_role(value: &str) -> Result<CampaignRole, IdentityError> {
    match value {
        "CAMPAIGN_OWNER" => Ok(CampaignRole::CampaignOwner),
        "HUMAN_KEEPER" => Ok(CampaignRole::HumanKeeper),
        "PLAYER" => Ok(CampaignRole::Player),
        "SPECTATOR" => Ok(CampaignRole::Spectator),
        _ => Err(IdentityError::InvalidIdentityData),
    }
}

fn authority_mode_name(mode: &AuthorityMode) -> &'static str {
    match mode {
        AuthorityMode::HumanKp => "HUMAN_KP",
        AuthorityMode::AiKp => "AI_KP",
    }
}

fn parse_authority_mode(value: &str) -> Result<AuthorityMode, IdentityError> {
    match value {
        "HUMAN_KP" => Ok(AuthorityMode::HumanKp),
        "AI_KP" => Ok(AuthorityMode::AiKp),
        _ => Err(IdentityError::InvalidIdentityData),
    }
}

fn timestamp_from_i64(value: i64) -> Result<u64, IdentityError> {
    u64::try_from(value).map_err(|_| IdentityError::InvalidIdentityData)
}

fn map_postgres_error(_error: postgres::Error) -> IdentityError {
    IdentityError::PersistenceUnavailable
}

fn apply_identity_migrations(database: &mut Client) -> Result<(), IdentityError> {
    for (_, migration) in schema::migration_statements() {
        database
            .batch_execute(migration)
            .map_err(|_| IdentityError::PersistenceUnavailable)?;
    }
    Ok(())
}

fn persist_session(
    database: &mut impl GenericClient,
    token_hash: [u8; 32],
    record: &SessionRecord,
    rotated_from: Option<&EntityId>,
) -> Result<(), IdentityError> {
    let issued_at =
        i64::try_from(record.issued_at_unix_ms).map_err(|_| IdentityError::InvalidIdentityData)?;
    let expires_at =
        i64::try_from(record.expires_at_unix_ms).map_err(|_| IdentityError::InvalidIdentityData)?;
    let rotated_from = rotated_from.map(EntityId::as_str);
    database
        .execute(
            "INSERT INTO sessions (\
                session_id, user_id, token_hash, issued_at, expires_at, rotated_from_session_id\
             ) VALUES (\
                $1, $2, $3, to_timestamp($4::bigint / 1000.0), \
                to_timestamp($5::bigint / 1000.0), $6\
             )",
            &[
                &record.session_id.as_str(),
                &record.user_id.as_str(),
                &&token_hash[..],
                &issued_at,
                &expires_at,
                &rotated_from,
            ],
        )
        .map_err(map_postgres_error)?;
    Ok(())
}

fn validate_password(password: &str) -> Result<(), IdentityError> {
    if password.len() < 12 || password.len() > 1024 {
        return Err(IdentityError::InvalidIdentityData);
    }
    Ok(())
}

fn uses_local_plaintext_postgres_transport(config: &PostgresConfig) -> Result<bool, IdentityError> {
    let local = config.get_hosts().iter().all(|host| match host {
        PostgresHost::Tcp(host) => matches!(host.as_str(), "localhost" | "127.0.0.1" | "::1"),
        #[cfg(unix)]
        PostgresHost::Unix(_) => true,
    });
    if !local && !matches!(config.get_ssl_mode(), PostgresSslMode::Require) {
        return Err(IdentityError::PersistenceUnavailable);
    }
    Ok(local && !matches!(config.get_ssl_mode(), PostgresSslMode::Require))
}

fn parse_postgres_config(
    database_url: &str,
    ca_certificate_pem: Option<&[u8]>,
) -> Result<PostgresConfig, IdentityError> {
    if let Ok(config) = database_url.parse::<PostgresConfig>() {
        return Ok(config);
    }

    // `tokio-postgres` accepts only disable/prefer/require and does not parse
    // libpq's sslrootcert option. Production URLs are also consumed by SQLx,
    // so preserve their verify-full form at the secret boundary and normalize
    // only the two TLS options whose guarantees are implemented below by
    // native-tls (verified chain plus hostname validation).
    let mut url =
        url::Url::parse(database_url).map_err(|_| IdentityError::PersistenceUnavailable)?;
    if !matches!(url.scheme(), "postgres" | "postgresql") {
        return Err(IdentityError::PersistenceUnavailable);
    }
    let pairs = url
        .query_pairs()
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();
    let sslmode_count = pairs.iter().filter(|(key, _)| key == "sslmode").count();
    let sslrootcert_count = pairs.iter().filter(|(key, _)| key == "sslrootcert").count();
    if sslmode_count != 1 || sslrootcert_count > 1 {
        return Err(IdentityError::PersistenceUnavailable);
    }

    let mut normalized_verify_full = false;
    let mut explicit_root_certificate = false;
    {
        let mut query = url.query_pairs_mut();
        query.clear();
        for (key, value) in pairs {
            match (key.as_str(), value.as_str()) {
                ("sslmode", "verify-full") => {
                    query.append_pair("sslmode", "require");
                    normalized_verify_full = true;
                }
                ("sslrootcert", _) => {
                    explicit_root_certificate = true;
                }
                _ => {
                    query.append_pair(&key, &value);
                }
            }
        }
    }
    if !normalized_verify_full || (explicit_root_certificate && ca_certificate_pem.is_none()) {
        return Err(IdentityError::PersistenceUnavailable);
    }
    url.as_str()
        .parse::<PostgresConfig>()
        .map_err(|_| IdentityError::PersistenceUnavailable)
}

fn connect_postgres(
    database_url: &str,
    ca_certificate_pem: Option<&[u8]>,
) -> Result<Client, IdentityError> {
    let config = parse_postgres_config(database_url, ca_certificate_pem)?;
    if uses_local_plaintext_postgres_transport(&config)? {
        return config
            .connect(NoTls)
            .map_err(|_| IdentityError::PersistenceUnavailable);
    }

    let mut connector = TlsConnector::builder();
    connector.min_protocol_version(Some(Protocol::Tlsv12));
    if let Some(certificate) = ca_certificate_pem {
        connector.add_root_certificate(
            Certificate::from_pem(certificate)
                .map_err(|_| IdentityError::PersistenceUnavailable)?,
        );
    }
    let connector = connector
        .build()
        .map_err(|_| IdentityError::PersistenceUnavailable)?;
    config
        .connect(MakeTlsConnector::new(connector))
        .map_err(|_| IdentityError::PersistenceUnavailable)
}

#[cfg(test)]
fn connect_local_postgres(database_url: &str) -> Result<Client, IdentityError> {
    connect_postgres(database_url, None)
}

fn hash_token(token: &str) -> [u8; 32] {
    Sha256::digest(token.as_bytes()).into()
}

fn login_rate_limit_key(login: &str) -> String {
    hex_encode(&Sha256::digest(
        login.trim().to_ascii_lowercase().as_bytes(),
    ))
}

fn verify_password(password: &str, encoded_hash: &str) -> Result<bool, IdentityError> {
    let parsed_hash =
        PasswordHash::new(encoded_hash).map_err(|_| IdentityError::PasswordHashFailure)?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

fn parse_timestamp(value: &str) -> Result<u64, IdentityError> {
    value
        .parse::<u64>()
        .map_err(|_| IdentityError::InvalidInternalCredential)
}

fn ensure_internal_time(
    now_unix_ms: u64,
    issued_at_unix_ms: u64,
    expires_at_unix_ms: u64,
) -> Result<(), IdentityError> {
    if issued_at_unix_ms > now_unix_ms || expires_at_unix_ms <= now_unix_ms {
        return Err(IdentityError::InternalCredentialExpired);
    }
    Ok(())
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn hex_decode_32(value: &str) -> Result<[u8; 32], IdentityError> {
    if value.len() != 64 {
        return Err(IdentityError::InvalidInternalCredential);
    }
    let mut output = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        output[index] = (hex_value(pair[0])? << 4) | hex_value(pair[1])?;
    }
    Ok(output)
}

fn hex_value(value: u8) -> Result<u8, IdentityError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err(IdentityError::InvalidInternalCredential),
    }
}
