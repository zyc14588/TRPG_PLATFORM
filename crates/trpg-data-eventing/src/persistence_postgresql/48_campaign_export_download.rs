#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CampaignExportDownloadAuthorization {
    pub token: String,
    pub expires_at_unix_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CampaignExportDownloadArtifact {
    pub artifact_key: String,
    pub artifact_hash: String,
}

impl CoreDomainRepository {
    pub async fn issue_campaign_export_download_for_actor(
        &self,
        actor_id: &str,
        include_all: bool,
        campaign_id: &str,
        export_id: &str,
        now_unix_ms: u64,
        ttl_ms: u64,
    ) -> Result<CampaignExportDownloadAuthorization, CoreDomainRepositoryError> {
        if !(1_000..=300_000).contains(&ttl_ms) {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "campaign_export_download_ttl",
            ));
        }
        let now = timestamp_from_unix_ms(now_unix_ms, "campaign_export_download.now")?;
        let expires_at_unix_ms = now_unix_ms
            .checked_add(ttl_ms)
            .and_then(|value| i64::try_from(value).ok())
            .ok_or(CoreDomainRepositoryError::InvalidInput(
                "campaign_export_download_expiry",
            ))?;
        let expires_at = timestamp_from_unix_ms(
            u64::try_from(expires_at_unix_ms).map_err(|_| {
                CoreDomainRepositoryError::InvalidInput("campaign_export_download_expiry")
            })?,
            "campaign_export_download.expiry",
        )?;

        let authorized: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1
                  FROM public.campaign_exports AS export
                  JOIN public.campaign_export_jobs AS job
                    ON job.export_id = export.export_id
                 WHERE export.export_id = $4
                   AND export.campaign_id = $3
                   AND job.state = 'READY'
                   AND job.retention_expires_at > $5
                   AND (
                        $2
                        OR (export.audience = 'PLAYER' AND export.requested_by = $1)
                        OR EXISTS(
                            SELECT 1
                              FROM public.campaign_memberships AS membership
                             WHERE membership.campaign_id = export.campaign_id
                               AND membership.user_id = $1
                               AND membership.role IN ('CAMPAIGN_OWNER', 'HUMAN_KEEPER')
                               AND membership.revoked_at IS NULL
                        )
                   )
            )
            "#,
        )
        .bind(actor_id)
        .bind(include_all)
        .bind(campaign_id)
        .bind(export_id)
        .bind(now)
        .fetch_one(&self.primary)
        .await
        .map_err(database_error("authorize_campaign_export_download"))?;
        if !authorized {
            return Err(CoreDomainRepositoryError::NotFound("campaign_export"));
        }

        let mut token_bytes = [0_u8; 32];
        SystemRandom::new()
            .fill(&mut token_bytes)
            .map_err(|_| CoreDomainRepositoryError::Integrity("download_token_random"))?;
        let token = hex_bytes(&token_bytes);
        let token_hash = sha256_prefixed(token.as_bytes());
        token_bytes.fill(0);

        let mut transaction = self
            .primary
            .begin()
            .await
            .map_err(database_error("begin_campaign_export_download_ticket"))?;
        sqlx::query(
            "DELETE FROM public.campaign_export_download_tickets WHERE expires_at <= $1",
        )
        .bind(now)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("expire_campaign_export_download_tickets"))?;
        sqlx::query(
            r#"
            INSERT INTO public.campaign_export_download_tickets (
                token_hash, export_id, actor_id, expires_at
            ) VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(token_hash)
        .bind(export_id)
        .bind(actor_id)
        .bind(expires_at)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("insert_campaign_export_download_ticket"))?;
        transaction
            .commit()
            .await
            .map_err(database_error("commit_campaign_export_download_ticket"))?;
        Ok(CampaignExportDownloadAuthorization {
            token,
            expires_at_unix_ms,
        })
    }

    pub async fn consume_campaign_export_download_for_actor(
        &self,
        actor_id: &str,
        campaign_id: &str,
        export_id: &str,
        token: &str,
        now_unix_ms: u64,
    ) -> Result<CampaignExportDownloadArtifact, CoreDomainRepositoryError> {
        if token.len() != 64 || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(CoreDomainRepositoryError::NotFound(
                "campaign_export_download",
            ));
        }
        let now = timestamp_from_unix_ms(now_unix_ms, "campaign_export_download.now")?;
        let token_hash = sha256_prefixed(token.as_bytes());
        let mut transaction = self
            .primary
            .begin()
            .await
            .map_err(database_error("begin_consume_campaign_export_download"))?;
        let ticket_export_id = sqlx::query_scalar::<_, String>(
            r#"
            DELETE FROM public.campaign_export_download_tickets AS ticket
             USING public.campaign_export_jobs AS job
             WHERE ticket.token_hash = $1
               AND ticket.export_id = $2
               AND ticket.actor_id = $3
               AND ticket.expires_at > $4
               AND job.export_id = ticket.export_id
               AND job.campaign_id = $5
               AND job.state = 'READY'
               AND job.retention_expires_at > $4
            RETURNING ticket.export_id
            "#,
        )
        .bind(token_hash)
        .bind(export_id)
        .bind(actor_id)
        .bind(now)
        .bind(campaign_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(database_error("consume_campaign_export_download_ticket"))?
        .ok_or(CoreDomainRepositoryError::NotFound(
            "campaign_export_download",
        ))?;
        let row = sqlx::query(
            r#"
            SELECT artifact_key, artifact_hash
              FROM public.campaign_export_jobs
             WHERE export_id = $1 AND campaign_id = $2 AND state = 'READY'
            "#,
        )
        .bind(ticket_export_id)
        .bind(campaign_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(database_error("load_campaign_export_download_artifact"))?;
        transaction
            .commit()
            .await
            .map_err(database_error("commit_consume_campaign_export_download"))?;
        Ok(CampaignExportDownloadArtifact {
            artifact_key: row.get("artifact_key"),
            artifact_hash: row.get("artifact_hash"),
        })
    }
}

fn sha256_prefixed(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn hex_bytes(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}
