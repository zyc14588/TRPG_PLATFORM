#[test]
fn v1_openapi_document_keeps_the_minimum_lifecycle_surface() {
    let document = trpg_api::api_contracts::v1_openapi_document();
    assert_eq!(document["openapi"], "3.1.0");
    assert_eq!(document["info"]["version"], "1.0.0");

    let paths = document["paths"]
        .as_object()
        .expect("v1 OpenAPI paths must be an object");
    for path in [
        "/api/v1/campaigns",
        "/api/v1/campaigns/{campaign_id}",
        "/api/v1/campaigns/{campaign_id}/invites",
        "/api/v1/campaigns/{campaign_id}/invites/{invite_id}/accept",
        "/api/v1/campaigns/{campaign_id}/characters",
        "/api/v1/campaigns/{campaign_id}/characters/{character_id}",
        "/api/v1/campaigns/{campaign_id}/characters/{character_id}/submit",
        "/api/v1/campaigns/{campaign_id}/characters/{character_id}/review",
        "/api/v1/campaigns/{campaign_id}/sessions",
        "/api/v1/campaigns/{campaign_id}/sessions/{session_id}",
        "/api/v1/campaigns/{campaign_id}/sessions/{session_id}/scenes",
        "/api/v1/campaigns/{campaign_id}/sessions/{session_id}/characters/{character_id}/join",
        "/api/v1/campaigns/{campaign_id}/player-actions",
        "/api/v1/campaigns/{campaign_id}/reconsiderations",
        "/api/v1/campaigns/{campaign_id}/forks",
        "/api/v1/campaigns/{campaign_id}/exports",
        "/api/v1/campaigns/{campaign_id}/exports/{export_id}",
    ] {
        assert!(paths.contains_key(path), "missing stable v1 path {path}");
        for parameter_name in path
            .split(['{', '}'])
            .skip(1)
            .step_by(2)
            .filter(|name| !name.is_empty())
        {
            let declared = paths[path]["parameters"]
                .as_array()
                .expect("templated paths must declare path parameters")
                .iter()
                .any(|parameter| {
                    parameter["name"] == parameter_name
                        && parameter["in"] == "path"
                        && parameter["required"] == true
                });
            assert!(
                declared,
                "path {path} does not declare required parameter {parameter_name}"
            );
        }
    }

    let command = &document["components"]["schemas"]["ApiCommandFields"];
    let required = command["required"]
        .as_array()
        .expect("ApiCommandFields.required must be an array");
    for field in ["command_id", "idempotency_key", "expected_version"] {
        assert!(
            required.iter().any(|value| value == field),
            "missing command field {field}"
        );
    }
}
