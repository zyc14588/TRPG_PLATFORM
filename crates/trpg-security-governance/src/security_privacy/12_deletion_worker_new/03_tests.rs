
#[cfg(test)]
mod tests {
    use super::*;
    use aws_sdk_s3::types::VersioningConfiguration;

    #[test]
    fn deletion_execution_context_debug_redacts_the_claim_capability() {
        let context = DeletionExecutionContext::new(
            "deletion_job",
            "data_subject",
            "claim-capability-must-not-be-logged".to_owned(),
        );
        let debug = format!("{context:?}");

        assert!(debug.contains("deletion_job"));
        assert!(debug.contains("data_subject"));
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("claim-capability-must-not-be-logged"));
    }

    #[test]
    fn legacy_queue_message_is_classified_from_its_data_subject() {
        let payload = br#"{"data_subject_id":"victim_subject","payload":{"private":"value"}}"#;
        assert_eq!(
            retained_message_subject_digest(None, payload).unwrap(),
            format!("sha256:{}", sha256_hex(b"victim_subject"))
        );
    }

    #[test]
    fn queue_absence_proof_rejects_unclassified_or_mismatched_messages() {
        assert_eq!(
            retained_message_subject_digest(None, br#"{"payload":"unclassified"}"#),
            Err(PrivacyError::InvalidPersistedState)
        );
        assert_eq!(
            retained_message_subject_digest(
                Some("sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
                br#"{"data_subject_id":"victim_subject"}"#,
            ),
            Err(PrivacyError::InvalidPersistedState)
        );
        assert_eq!(
            retained_message_subject_digest(
                Some("sha256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"),
                br#"{"payload":"unclassified"}"#,
            ),
            Err(PrivacyError::InvalidPersistedState)
        );
    }

    #[test]
    fn service_credentials_require_tls_and_are_removed_from_nats_endpoint() {
        assert!(build_redis_client(
            "redis://localhost:6379",
            Some(b"ca"),
            Some(b"certificate"),
            Some(b"private-key"),
        )
        .is_err());
        let (endpoint, credentials) = nats_endpoint_and_credentials(
            "tls://runtime%20user:secret%40value@nats.example.invalid:4222",
        )
        .unwrap();
        assert_eq!(endpoint.as_str(), "tls://nats.example.invalid:4222");
        assert_eq!(
            credentials,
            Some(("runtime user".to_owned(), "secret@value".to_owned()))
        );
    }

    #[test]
    fn ipv6_loopback_is_the_only_cleartext_ipv6_service_host() {
        assert_eq!(
            validate_secure_service_url("http://[::1]:8080", "http", "https"),
            Ok(())
        );
        assert_eq!(
            validate_secure_service_url("http://[::2]:8080", "http", "https"),
            Err(PrivacyError::InvalidInput)
        );
    }

    #[test]
    fn object_storage_refuses_plaintext_before_loading_credentials() {
        assert_eq!(
            validate_s3_tls_binding(
                "http://127.0.0.1:9000",
                Path::new("/does/not/need/to/exist")
            ),
            Err(PrivacyError::InvalidInput)
        );
    }

    #[tokio::test]
    #[ignore = "requires the isolated AR02 MinIO TLS fixture"]
    async fn ar02_live_s3_version_erasure_closes_recoverable_history() {
        fn required(name: &str) -> String {
            std::env::var(name).unwrap_or_else(|_| panic!("{name} is required"))
        }

        fn client(endpoint: &str, region: &str, access: &str, secret: &str) -> S3Client {
            let config = aws_sdk_s3::Config::builder()
                .behavior_version(BehaviorVersion::latest())
                .region(Region::new(region.to_owned()))
                .credentials_provider(Credentials::new(
                    access,
                    secret,
                    None,
                    None,
                    "ar02-live-s3-fixture",
                ))
                .endpoint_url(endpoint)
                .force_path_style(true)
                .build();
            S3Client::from_conf(config)
        }

        async fn set_versioning(
            client: &S3Client,
            bucket: &str,
            status: BucketVersioningStatus,
        ) {
            client
                .put_bucket_versioning()
                .bucket(bucket)
                .versioning_configuration(
                    VersioningConfiguration::builder().status(status).build(),
                )
                .send()
                .await
                .expect("root fixture can set bucket versioning");
        }

        async fn put(
            client: &S3Client,
            bucket: &str,
            key: &str,
            payload: &'static [u8],
        ) -> String {
            client
                .put_object()
                .bucket(bucket)
                .key(key)
                .body(ByteStream::from_static(payload))
                .send()
                .await
                .expect("root fixture can write a version");
            client
                .list_object_versions()
                .bucket(bucket)
                .prefix(key)
                .send()
                .await
                .expect("root fixture can enumerate the written version")
                .versions()
                .iter()
                .find(|version| version.key() == Some(key) && version.is_latest() == Some(true))
                .and_then(|version| version.version_id())
                .expect("version listing returns the authoritative version id")
                .to_owned()
        }

        let endpoint = required("AR02_MINIO_ENDPOINT");
        let region = required("AR02_MINIO_REGION");
        let bucket = required("AR02_MINIO_BUCKET");
        let root_access = required("AR02_MINIO_ROOT_ACCESS_KEY");
        let root_secret = required("AR02_MINIO_ROOT_SECRET_KEY");
        let service_access = required("AR02_MINIO_SERVICE_ACCESS_KEY");
        let service_secret = required("AR02_MINIO_SERVICE_SECRET_KEY");
        let ca_path = PathBuf::from(required("AR02_MINIO_CA_CERT_PATH"));
        assert_eq!(
            std::fs::canonicalize(&ca_path).unwrap(),
            std::fs::canonicalize(required("SSL_CERT_FILE")).unwrap()
        );

        let root = client(&endpoint, &region, &root_access, &root_secret);
        let root_surface = S3ObjectDeletionSurface::connect(
            &endpoint,
            &region,
            &bucket,
            &root_access,
            &root_secret,
            &ca_path,
        )
        .await;
        assert_eq!(
            root_surface.unwrap_err(),
            PrivacyError::InvalidInput,
            "application startup must reject root/admin credentials"
        );

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let subject_id = format!("ar02_subject_{nonce}");
        let prefix = S3ObjectDeletionSurface::subject_prefix(&subject_id).unwrap();
        set_versioning(&root, &bucket, BucketVersioningStatus::Enabled).await;

        let historical_key = format!("{prefix}historical");
        let historical_v1 = put(&root, &bucket, &historical_key, b"historical-v1").await;
        let historical_v2 = put(&root, &bucket, &historical_key, b"historical-v2").await;
        root.delete_object()
            .bucket(&bucket)
            .key(&historical_key)
            .send()
            .await
            .expect("root fixture can create a delete marker");
        let marker_id = root
            .list_object_versions()
            .bucket(&bucket)
            .prefix(&historical_key)
            .send()
            .await
            .expect("root fixture can enumerate the delete marker")
            .delete_markers()
            .iter()
            .find(|marker| {
                marker.key() == Some(historical_key.as_str())
                    && marker.is_latest() == Some(true)
            })
            .and_then(|marker| marker.version_id())
            .expect("version listing returns the authoritative marker id")
            .to_owned();
        let mut known_versions = vec![
            (historical_key.clone(), historical_v1),
            (historical_key.clone(), historical_v2),
        ];
        for index in 0..101 {
            let key = format!("{prefix}paged-{index:03}");
            let version_id = put(&root, &bucket, &key, b"paged-version").await;
            if index == 100 {
                known_versions.push((key, version_id));
            }
        }

        set_versioning(&root, &bucket, BucketVersioningStatus::Suspended).await;
        let null_key = format!("{prefix}null-version");
        let null_version = put(&root, &bucket, &null_key, b"suspended-null-version").await;
        assert_eq!(null_version, "null");
        known_versions.push((null_key, null_version));

        let surface = S3ObjectDeletionSurface::connect(
            &endpoint,
            &region,
            &bucket,
            &service_access,
            &service_secret,
            &ca_path,
        )
        .await
        .expect("least-privilege service identity can initialize the S3 surface");
        surface
            .erase_subject(&subject_id)
            .await
            .expect("all historical versions and delete markers are permanently erased");
        assert!(surface.verify_absent(&subject_id).await.unwrap());
        let evidence = surface
            .last_erasure_evidence()
            .unwrap()
            .expect("erasure emits structured non-secret evidence");
        assert_eq!(evidence.bucket, bucket);
        assert_eq!(evidence.prefix, prefix);
        assert!(
            evidence.page_count >= 3,
            "more than one listing page plus final verification is required"
        );
        assert_eq!(evidence.version_count, 105);
        assert_eq!(evidence.delete_receipt_summary.error_count, 0);
        assert_eq!(
            evidence.delete_receipt_summary.requested_count,
            evidence.delete_receipt_summary.confirmed_count
        );
        assert_eq!(evidence.manifest_sha256.len(), 64);

        for (key, version_id) in known_versions {
            assert!(
                root.get_object()
                    .bucket(&bucket)
                    .key(key)
                    .version_id(version_id)
                    .send()
                    .await
                    .is_err(),
                "known historical version must no longer be readable"
            );
        }
        let remaining = root
            .list_object_versions()
            .bucket(&bucket)
            .prefix(&prefix)
            .send()
            .await
            .expect("root fixture can verify the complete version namespace");
        assert!(remaining.versions().is_empty());
        assert!(remaining.delete_markers().is_empty());
        assert!(
            !remaining.is_truncated().unwrap_or(true),
            "final root verification must be complete"
        );
        assert!(
            root.get_object()
                .bucket(&bucket)
                .key(&historical_key)
                .version_id(marker_id)
                .send()
                .await
                .is_err(),
            "known delete marker must no longer exist"
        );

        set_versioning(&root, &bucket, BucketVersioningStatus::Enabled).await;
        surface
            .put_protected_object(&subject_id, "legitimate-after-erasure", b"ciphertext")
            .await
            .expect("least-privilege production write remains available when versioning is enabled");
        surface
            .erase_subject(&subject_id)
            .await
            .expect("enabled-version erasure remains idempotent after a legitimate new write");
        assert!(surface.verify_absent(&subject_id).await.unwrap());
    }
}
