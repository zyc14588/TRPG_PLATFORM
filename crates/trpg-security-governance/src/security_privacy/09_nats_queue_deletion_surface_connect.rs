
impl NatsQueueDeletionSurface {
    pub async fn connect(
        nats_url: &str,
        stream_name: &str,
        subject_prefix: &str,
    ) -> Result<Self, PrivacyError> {
        let surface = Self::connect_context(
            nats_url,
            stream_name,
            subject_prefix,
            None,
            None,
            None,
            None,
        )
        .await?;
        surface
            .jetstream
            .get_stream(&surface.stream_name)
            .await
            .map_err(|_| PrivacyError::Queue)?;
        Ok(surface)
    }

    pub async fn connect_or_create_for_test(
        nats_url: &str,
        stream_name: &str,
        subject_prefix: &str,
    ) -> Result<Self, PrivacyError> {
        let surface = Self::connect_context(
            nats_url,
            stream_name,
            subject_prefix,
            None,
            None,
            None,
            None,
        )
        .await?;
        surface
            .jetstream
            .get_or_create_stream(async_nats::jetstream::stream::Config {
                name: surface.stream_name.clone(),
                subjects: vec![format!("{}.*", surface.subject_prefix)],
                ..Default::default()
            })
            .await
            .map_err(|_| PrivacyError::Queue)?;
        Ok(surface)
    }

    async fn connect_context(
        nats_url: &str,
        stream_name: &str,
        subject_prefix: &str,
        ca_certificate_path: Option<&Path>,
        client_certificate_path: Option<&Path>,
        client_private_key_path: Option<&Path>,
        credentials_path: Option<&Path>,
    ) -> Result<Self, PrivacyError> {
        validate_secure_service_url(nats_url, "nats", "tls")?;
        validate_id(stream_name)?;
        if subject_prefix.is_empty()
            || subject_prefix.len() > 128
            || !subject_prefix.split('.').all(|token| {
                !token.is_empty()
                    && token
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
            })
        {
            return Err(PrivacyError::InvalidInput);
        }
        if client_certificate_path.is_some() != client_private_key_path.is_some() {
            return Err(PrivacyError::InvalidInput);
        }
        let (connection_url, url_credentials) = nats_endpoint_and_credentials(nats_url)?;
        if credentials_path.is_some() && url_credentials.is_some() {
            return Err(PrivacyError::InvalidInput);
        }
        let tls_nats = connection_url.scheme() == "tls";
        let mut options = async_nats::ConnectOptions::new().require_tls(tls_nats);
        if tls_nats {
            options = options.tls_first();
        }
        if let Some(path) = ca_certificate_path {
            options = options.add_root_certificates(path.to_path_buf());
        }
        if let (Some(certificate), Some(private_key)) =
            (client_certificate_path, client_private_key_path)
        {
            options = options
                .add_client_certificate(certificate.to_path_buf(), private_key.to_path_buf());
        }
        if let Some((username, password)) = url_credentials {
            options = options.user_and_password(username, password);
        } else if let Some(path) = credentials_path {
            options = options
                .credentials_file(path)
                .await
                .map_err(|_| PrivacyError::InvalidInput)?;
        }
        let client = options
            .connect(connection_url.as_str())
            .await
            .map_err(|_| PrivacyError::Queue)?;
        Ok(Self {
            jetstream: async_nats::jetstream::new(client),
            stream_name: stream_name.to_owned(),
            subject_prefix: subject_prefix.to_owned(),
            canonical_pool: None,
        })
    }

    /// Production queue deletion dead-letters unpublished subject rows and
    /// removes every already-published message whose server-authored subject
    /// digest matches the erased subject. Canonical PostgreSQL history remains
    /// append-only, while the delivery surface is proved byte-absent.
    pub async fn connect_crypto_erasure(
        nats_url: &str,
        stream_name: &str,
        canonical_pool: PgPool,
    ) -> Result<Self, PrivacyError> {
        Self::connect_crypto_erasure_with_credentials(
            nats_url,
            stream_name,
            canonical_pool,
            None,
            None,
            None,
            None,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn connect_crypto_erasure_with_credentials(
        nats_url: &str,
        stream_name: &str,
        canonical_pool: PgPool,
        ca_certificate_path: Option<&Path>,
        client_certificate_path: Option<&Path>,
        client_private_key_path: Option<&Path>,
        credentials_path: Option<&Path>,
    ) -> Result<Self, PrivacyError> {
        let mut surface = Self::connect_context(
            nats_url,
            stream_name,
            "trpg.events",
            ca_certificate_path,
            client_certificate_path,
            client_private_key_path,
            credentials_path,
        )
        .await?;
        surface
            .jetstream
            .get_stream(&surface.stream_name)
            .await
            .map_err(|_| PrivacyError::Queue)?;
        surface.canonical_pool = Some(canonical_pool);
        Ok(surface)
    }

    pub async fn put_for_test(
        &self,
        subject_id: &str,
        protected_payload: &[u8],
    ) -> Result<(), PrivacyError> {
        validate_id(subject_id)?;
        if protected_payload.is_empty() {
            return Err(PrivacyError::InvalidInput);
        }
        self.jetstream
            .publish(self.subject(subject_id), protected_payload.to_vec().into())
            .await
            .map_err(|_| PrivacyError::Queue)?
            .await
            .map_err(|_| PrivacyError::Queue)?;
        Ok(())
    }

    pub async fn put_canonical_for_test(
        &self,
        subject_id: &str,
        protected_payload: &[u8],
    ) -> Result<(), PrivacyError> {
        validate_id(subject_id)?;
        if protected_payload.is_empty() {
            return Err(PrivacyError::InvalidInput);
        }
        let mut headers = async_nats::HeaderMap::new();
        headers.insert(
            "Trpg-Data-Subject-Digest",
            format!("sha256:{}", sha256_hex(subject_id.as_bytes())),
        );
        self.jetstream
            .publish_with_headers(
                self.canonical_subject(subject_id),
                headers,
                protected_payload.to_vec().into(),
            )
            .await
            .map_err(|_| PrivacyError::Queue)?
            .await
            .map_err(|_| PrivacyError::Queue)?;
        Ok(())
    }

    pub async fn cleanup_for_test(&self) -> Result<(), PrivacyError> {
        self.jetstream
            .delete_stream(&self.stream_name)
            .await
            .map_err(|_| PrivacyError::Queue)?;
        Ok(())
    }

    fn subject(&self, subject_id: &str) -> String {
        format!("{}.{}", self.subject_prefix, subject_id)
    }

    fn canonical_subject(&self, subject_id: &str) -> String {
        format!(
            "{}.appended.subject.{}",
            self.subject_prefix,
            sha256_hex(subject_id.as_bytes())
        )
    }

    async fn canonical_subject_message_batch(
        &self,
        subject_id: &str,
        cursor: u64,
        limit: usize,
    ) -> Result<(Vec<u64>, u64, bool), PrivacyError> {
        if cursor == 0 || !(1..=NATS_DELETION_BATCH_SIZE).contains(&limit) {
            return Err(PrivacyError::InvalidPersistedState);
        }
        let stream = self
            .jetstream
            .get_stream(&self.stream_name)
            .await
            .map_err(|_| PrivacyError::Queue)?;
        let legacy_subject = format!("{}.appended", self.subject_prefix);
        match stream
            .get_last_raw_message_by_subject(&legacy_subject)
            .await
        {
            Ok(_) => return Err(PrivacyError::InvalidPersistedState),
            Err(error)
                if error.kind()
                    == async_nats::jetstream::stream::LastRawMessageErrorKind::NoMessageFound => {}
            Err(_) => return Err(PrivacyError::Queue),
        }

        let canonical_subject = self.canonical_subject(subject_id);
        let expected = format!("sha256:{}", sha256_hex(subject_id.as_bytes()));
        let mut matches = Vec::new();
        let mut next_sequence = cursor;
        while matches.len() < limit {
            match stream
                .get_first_raw_message_by_subject(&canonical_subject, next_sequence)
                .await
            {
                Ok(message) => {
                    let header_digest = message
                        .headers
                        .get("Trpg-Data-Subject-Digest")
                        .map(|value| value.as_str());
                    let observed =
                        retained_message_subject_digest(header_digest, &message.payload)?;
                    if observed == expected {
                        matches.push(message.sequence);
                    } else {
                        return Err(PrivacyError::InvalidPersistedState);
                    }
                    next_sequence = message
                        .sequence
                        .checked_add(1)
                        .ok_or(PrivacyError::InvalidPersistedState)?;
                }
                Err(error)
                    if error.kind()
                        == async_nats::jetstream::stream::RawMessageErrorKind::NoMessageFound =>
                {
                    return Ok((matches, next_sequence, true));
                }
                Err(_) => return Err(PrivacyError::Queue),
            }
        }
        Ok((matches, next_sequence, false))
    }
}

fn nats_endpoint_and_credentials(
    nats_url: &str,
) -> Result<(Url, Option<(String, String)>), PrivacyError> {
    let mut endpoint = Url::parse(nats_url).map_err(|_| PrivacyError::InvalidInput)?;
    let credentials = match (endpoint.username(), endpoint.password()) {
        ("", None) => None,
        (username, Some(password)) if !username.is_empty() && !password.is_empty() => {
            let username = percent_decode_str(username)
                .decode_utf8()
                .map_err(|_| PrivacyError::InvalidInput)?
                .into_owned();
            let password = percent_decode_str(password)
                .decode_utf8()
                .map_err(|_| PrivacyError::InvalidInput)?
                .into_owned();
            if username.is_empty() || password.is_empty() {
                return Err(PrivacyError::InvalidInput);
            }
            Some((username, password))
        }
        _ => return Err(PrivacyError::InvalidInput),
    };
    endpoint
        .set_username("")
        .map_err(|_| PrivacyError::InvalidInput)?;
    endpoint
        .set_password(None)
        .map_err(|_| PrivacyError::InvalidInput)?;
    Ok((endpoint, credentials))
}

fn retained_message_subject_digest(
    header_digest: Option<&str>,
    payload: &[u8],
) -> Result<String, PrivacyError> {
    let payload_subject = serde_json::from_slice::<Value>(payload)
        .ok()
        .and_then(|payload| {
            payload
                .get("data_subject_id")
                .and_then(Value::as_str)
                .map(str::to_owned)
        });
    let payload_digest = payload_subject
        .as_deref()
        .map(|subject| format!("sha256:{}", sha256_hex(subject.as_bytes())));
    match (header_digest, payload_digest.as_deref()) {
        (Some(header), Some(payload)) if header == payload => Ok(header.to_owned()),
        (Some(header), None)
            if header.len() == 71
                && header.starts_with("sha256:")
                && header[7..]
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()) =>
        {
            Ok(header.to_owned())
        }
        (None, Some(payload)) => Ok(payload.to_owned()),
        // An unclassified or inconsistently classified retained message makes
        // absence unprovable. Never convert it into a successful result.
        _ => Err(PrivacyError::InvalidPersistedState),
    }
}
