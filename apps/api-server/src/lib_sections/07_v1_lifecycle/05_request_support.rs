fn v1_path_body_mismatch() -> HttpResponse {
    HttpResponse::json(400, json!({"error": "CORE_API_PATH_BODY_MISMATCH"}))
}

fn campaign_export_download_token(request: &HttpRequest) -> Option<&str> {
    request
        .header("x-trpg-export-authorization")
        .filter(|token| token.len() == 64 && token.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

fn core_request_error(namespace: &str, suffix: &str) -> HttpResponse {
    let status = if suffix == "WORKFLOW_UNAVAILABLE" {
        503
    } else {
        400
    };
    HttpResponse::json(status, json!({"error": format!("{namespace}_{suffix}")}))
}
