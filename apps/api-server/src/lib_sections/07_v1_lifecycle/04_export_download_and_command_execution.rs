impl ApiApplication {
    fn v1_download_export(
        &self,
        request: &HttpRequest,
        campaign_id: &str,
        export_id: &str,
    ) -> HttpResponse {
        let (actor_id, _) = match self.v1_query_actor(request) {
            Ok(actor) => actor,
            Err(response) => return response,
        };
        let token = match campaign_export_download_token(request) {
            Some(token) => token,
            None => {
                return HttpResponse::json(
                    404,
                    json!({"error": "CAMPAIGN_EXPORT_DOWNLOAD_NOT_FOUND"}),
                )
            }
        };
        let now = match now_unix_ms() {
            Ok(now) => now,
            Err(response) => return response,
        };
        let (custody, api) = match self.v1_binding() {
            Ok(binding) => binding,
            Err(response) => return response,
        };
        let descriptor = match custody.runtime.lock() {
            Ok(runtime) => runtime.block_on(api.consume_campaign_export_download(
                &actor_id,
                campaign_id,
                export_id,
                token,
                now,
            )),
            Err(_) => return internal_error(),
        };
        let descriptor = match descriptor {
            Ok(descriptor) => descriptor,
            Err(error) => return player_action_api_error(error),
        };
        let Some(root) = custody.export_storage_root.as_deref() else {
            return HttpResponse::json(
                503,
                json!({"error": "CAMPAIGN_EXPORT_STORAGE_UNAVAILABLE"}),
            );
        };
        let path = match checked_artifact_path(root, &descriptor.artifact_key) {
            Ok(path) => path,
            Err(_) => return internal_error(),
        };
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(_) => {
                return HttpResponse::json(
                    503,
                    json!({"error": "CAMPAIGN_EXPORT_ARTIFACT_UNAVAILABLE"}),
                )
            }
        };
        if artifact_sha256(&bytes) != descriptor.artifact_hash {
            return HttpResponse::json(
                409,
                json!({"error": "CAMPAIGN_EXPORT_ARTIFACT_INTEGRITY_FAILED"}),
            );
        }
        match serde_json::from_slice(&bytes) {
            Ok(artifact) => HttpResponse::json(200, artifact),
            Err(_) => HttpResponse::json(
                409,
                json!({"error": "CAMPAIGN_EXPORT_ARTIFACT_INVALID"}),
            ),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn v1_run_command<R, F>(
        &self,
        request: &HttpRequest,
        campaign_id: &str,
        resource_type: &str,
        resource_id: &str,
        command: &ApiCommandFields,
        visibility: Visibility,
        allow_invite_acceptance: bool,
        success_status: u16,
        body: &R,
        run: F,
    ) -> HttpResponse
    where
        F: for<'a> FnOnce(
            &'a V1LifecycleApi<RepositoryCampaignCharacterPort>,
            &'a AuthorizedCoreApiContext,
            &'a R,
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<
                        Output = Result<trpg_api::api_contracts::CoreApiCommitReceipt, CoreApiError>,
                    > + 'a,
            >,
        >,
    {
        let now = match now_unix_ms() {
            Ok(now) => now,
            Err(response) => return response,
        };
        let context = match self.authorized_core_context(
            request,
            campaign_id,
            resource_type,
            resource_id,
            command,
            visibility,
            allow_invite_acceptance,
            "CORE_API",
            "authorize_v1_lifecycle_command",
            now,
        ) {
            Ok(context) => context,
            Err(response) => return response,
        };
        let (custody, api) = match self.v1_binding() {
            Ok(binding) => binding,
            Err(response) => return response,
        };
        let result = match custody.runtime.lock() {
            Ok(runtime) => runtime.block_on(run(&api, &context, body)),
            Err(_) => return internal_error(),
        };
        match result {
            Ok(receipt) => HttpResponse::json(
                success_status,
                json!({
                    "last_event_sequence": receipt.last_event_sequence,
                    "aggregate_version": receipt.aggregate_version,
                }),
            ),
            Err(error) => player_action_api_error(error),
        }
    }
}
