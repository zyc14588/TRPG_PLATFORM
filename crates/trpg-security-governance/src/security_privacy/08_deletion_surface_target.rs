
#[async_trait]
impl DeletionSurface for RedisCacheDeletionSurface {
    fn target(&self) -> DeletionTarget {
        DeletionTarget::Cache
    }

    async fn delete_subject_batch(
        &self,
        subject_id: &str,
        cursor: u64,
    ) -> Result<DeletionBatchProgress, PrivacyError> {
        if cursor != 1 {
            return Err(PrivacyError::InvalidPersistedState);
        }
        validate_id(subject_id)?;
        let mut connection = self.connection.clone();
        redis::Script::new(REDIS_DELETE_SUBJECT_KEYS)
            .key(self.subject_index_key(subject_id))
            .invoke_async::<i64>(&mut connection)
            .await
            .map(|_| ())
            .map_err(|_| PrivacyError::Cache)?;
        let independently_classified = self
            .independently_classified_subject_entries(subject_id)
            .await?;
        if !independently_classified.is_empty() {
            redis::cmd("DEL")
                .arg(independently_classified)
                .query_async::<i64>(&mut connection)
                .await
                .map_err(|_| PrivacyError::Cache)?;
        }
        DeletionBatchProgress::complete(cursor)
    }

    async fn verify_absent(&self, subject_id: &str) -> Result<bool, PrivacyError> {
        validate_id(subject_id)?;
        let mut connection = self.connection.clone();
        let count = redis::Script::new(REDIS_COUNT_SUBJECT_KEYS)
            .key(self.subject_index_key(subject_id))
            .invoke_async::<i64>(&mut connection)
            .await
            .map_err(|_| PrivacyError::Cache)?;
        if count != 0 {
            return Ok(false);
        }
        Ok(self
            .independently_classified_subject_entries(subject_id)
            .await?
            .is_empty())
    }
}

fn build_redis_client(
    redis_url: &str,
    root_certificate: Option<&[u8]>,
    client_certificate: Option<&[u8]>,
    client_private_key: Option<&[u8]>,
) -> Result<redis::Client, PrivacyError> {
    let material = [
        root_certificate.is_some(),
        client_certificate.is_some(),
        client_private_key.is_some(),
    ];
    if material.iter().any(|present| *present) && !material.iter().all(|present| *present) {
        return Err(PrivacyError::InvalidInput);
    }
    let tls = Url::parse(redis_url)
        .map_err(|_| PrivacyError::InvalidInput)?
        .scheme()
        == "rediss";
    if !tls && material.iter().any(|present| *present) {
        return Err(PrivacyError::InvalidInput);
    }
    if tls {
        let (root_certificate, client_certificate, client_private_key) = (
            root_certificate.ok_or(PrivacyError::InvalidInput)?,
            client_certificate.ok_or(PrivacyError::InvalidInput)?,
            client_private_key.ok_or(PrivacyError::InvalidInput)?,
        );
        redis::Client::build_with_tls(
            redis_url,
            redis::TlsCertificates {
                client_tls: Some(redis::ClientTlsConfig {
                    client_cert: client_certificate.to_vec(),
                    client_key: client_private_key.to_vec(),
                }),
                root_cert: Some(root_certificate.to_vec()),
            },
        )
        .map_err(|_| PrivacyError::InvalidInput)
    } else {
        redis::Client::open(redis_url).map_err(|_| PrivacyError::InvalidInput)
    }
}

fn sha256_hex(value: &[u8]) -> String {
    let digest = Sha256::digest(value);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[derive(Clone)]
pub struct NatsQueueDeletionSurface {
    jetstream: async_nats::jetstream::Context,
    stream_name: String,
    subject_prefix: String,
    canonical_pool: Option<PgPool>,
}

const NATS_DELETION_BATCH_SIZE: usize = 128;
