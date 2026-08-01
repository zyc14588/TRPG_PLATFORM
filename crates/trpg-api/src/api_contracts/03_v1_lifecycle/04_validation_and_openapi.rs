
fn validate_ids(values: &[&str]) -> Result<(), CoreApiError> {
    for value in values {
        EntityId::new(*value).map_err(|_| CoreApiError::InvalidInput("entity_id"))?;
    }
    Ok(())
}

/// Machine-readable, compatibility-tested surface for the minimum V1
/// lifecycle. The production handler publishes this exact document at
/// `/api/v1/openapi.json`.
pub fn v1_openapi_document() -> serde_json::Value {
    use serde_json::json;

    let command = |summary: &str| {
        json!({
            "summary": summary,
            "security": [{"bearerAuth": []}],
            "requestBody": {
                "required": true,
                "content": {
                    "application/json": {
                        "schema": {"type": "object"}
                    }
                }
            },
            "responses": {
                "200": {"description": "Canonical event receipt"},
                "201": {"description": "Canonical event receipt"},
                "202": {"description": "Canonical event receipt"},
                "400": {"description": "Invalid command"},
                "401": {"description": "Authentication required"},
                "403": {"description": "Policy denied"},
                "404": {"description": "Resource unavailable or invisible"},
                "409": {"description": "Idempotency or aggregate version conflict"}
            }
        })
    };
    let query = |summary: &str| {
        json!({
            "summary": summary,
            "security": [{"bearerAuth": []}],
            "responses": {
                "200": {"description": "Visible projection"},
                "401": {"description": "Authentication required"},
                "404": {"description": "Resource unavailable or invisible"}
            }
        })
    };
    let path_parameters = |names: &[&str]| {
        serde_json::Value::Array(
            names
                .iter()
                .map(|name| {
                    json!({
                        "name": name,
                        "in": "path",
                        "required": true,
                        "schema": {"type": "string", "minLength": 1}
                    })
                })
                .collect(),
        )
    };

    json!({
        "openapi": "3.1.0",
        "info": {
            "title": "COC AI TRPG V1 API",
            "version": "1.0.0"
        },
        "paths": {
            "/api/v1/campaigns": {
                "get": query("List visible Campaigns"),
                "post": command("Create Campaign")
            },
            "/api/v1/campaigns/{campaign_id}": {
                "parameters": path_parameters(&["campaign_id"]),
                "get": query("Get visible Campaign")
            },
            "/api/v1/campaigns/{campaign_id}/invites": {
                "parameters": path_parameters(&["campaign_id"]),
                "post": command("Issue Campaign invite")
            },
            "/api/v1/campaigns/{campaign_id}/invites/{invite_id}/accept": {
                "parameters": path_parameters(&["campaign_id", "invite_id"]),
                "post": command("Accept Campaign invite")
            },
            "/api/v1/campaigns/{campaign_id}/characters": {
                "parameters": path_parameters(&["campaign_id"]),
                "post": command("Create Character")
            },
            "/api/v1/campaigns/{campaign_id}/characters/{character_id}": {
                "parameters": path_parameters(&["campaign_id", "character_id"]),
                "put": command("Update Character draft")
            },
            "/api/v1/campaigns/{campaign_id}/characters/{character_id}/submit": {
                "parameters": path_parameters(&["campaign_id", "character_id"]),
                "post": command("Submit Character")
            },
            "/api/v1/campaigns/{campaign_id}/characters/{character_id}/review": {
                "parameters": path_parameters(&["campaign_id", "character_id"]),
                "post": command("Approve Character")
            },
            "/api/v1/campaigns/{campaign_id}/scenarios/import": {
                "parameters": path_parameters(&["campaign_id"]),
                "post": command("Import Scenario")
            },
            "/api/v1/campaigns/{campaign_id}/sessions": {
                "parameters": path_parameters(&["campaign_id"]),
                "post": command("Start Session and opening Scene")
            },
            "/api/v1/campaigns/{campaign_id}/sessions/{session_id}": {
                "parameters": path_parameters(&["campaign_id", "session_id"]),
                "patch": command("Change Session state")
            },
            "/api/v1/campaigns/{campaign_id}/sessions/{session_id}/scenes": {
                "parameters": path_parameters(&["campaign_id", "session_id"]),
                "post": command("Switch active Scene")
            },
            "/api/v1/campaigns/{campaign_id}/sessions/{session_id}/characters/{character_id}/join": {
                "parameters": path_parameters(&["campaign_id", "session_id", "character_id"]),
                "post": command("Join approved Character to Session")
            },
            "/api/v1/campaigns/{campaign_id}/player-actions": {
                "parameters": path_parameters(&["campaign_id"]),
                "post": command("Submit Player action")
            },
            "/api/v1/campaigns/{campaign_id}/player-actions/{action_id}/confirm": {
                "parameters": path_parameters(&["campaign_id", "action_id"]),
                "post": command("Confirm Player action")
            },
            "/api/v1/campaigns/{campaign_id}/reconsiderations": {
                "parameters": path_parameters(&["campaign_id"]),
                "post": command("Request reconsideration")
            },
            "/api/v1/campaigns/{campaign_id}/reconsiderations/{reconsideration_id}/review": {
                "parameters": path_parameters(&["campaign_id", "reconsideration_id"]),
                "post": command("Review reconsideration")
            },
            "/api/v1/campaigns/{campaign_id}/reconsiderations/{reconsideration_id}/resolve": {
                "parameters": path_parameters(&["campaign_id", "reconsideration_id"]),
                "post": command("Resolve reconsideration")
            },
            "/api/v1/campaigns/{campaign_id}/forks": {
                "parameters": path_parameters(&["campaign_id"]),
                "post": command("Fork Campaign from canonical Session history")
            },
            "/api/v1/campaigns/{campaign_id}/exports": {
                "parameters": path_parameters(&["campaign_id"]),
                "post": command("Request Campaign export")
            },
            "/api/v1/campaigns/{campaign_id}/exports/{export_id}": {
                "parameters": path_parameters(&["campaign_id", "export_id"]),
                "get": query("Get Campaign export status")
            },
            "/api/v1/campaigns/{campaign_id}/exports/{export_id}/download-authorizations": {
                "parameters": path_parameters(&["campaign_id", "export_id"]),
                "post": query("Issue one-time Campaign export download authorization")
            },
            "/api/v1/campaigns/{campaign_id}/exports/{export_id}/download": {
                "parameters": path_parameters(&["campaign_id", "export_id"]),
                "get": {
                    "description": "Consume one-time Campaign export download authorization",
                    "parameters": [{
                        "in": "header",
                        "name": "X-TRPG-Export-Authorization",
                        "required": true,
                        "schema": {"type": "string", "minLength": 64, "maxLength": 64}
                    }],
                    "responses": {"200": {"description": "OK"}},
                    "security": [{"bearerAuth": []}]
                }
            }
        },
        "components": {
            "securitySchemes": {
                "bearerAuth": {
                    "type": "http",
                    "scheme": "bearer"
                }
            },
            "schemas": {
                "ApiCommandFields": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": [
                        "command_id",
                        "idempotency_key",
                        "expected_version",
                        "correlation_id",
                        "causation_id",
                        "trace_id"
                    ],
                    "properties": {
                        "command_id": {"type": "string"},
                        "idempotency_key": {"type": "string", "maxLength": 160},
                        "expected_version": {"type": "integer", "minimum": 0},
                        "correlation_id": {"type": "string"},
                        "causation_id": {"type": "string"},
                        "trace_id": {"type": "string"}
                    }
                },
                "CoreApiCommitReceipt": {
                    "type": "object",
                    "required": ["last_event_sequence", "aggregate_version"],
                    "properties": {
                        "last_event_sequence": {"type": "integer"},
                        "aggregate_version": {"type": "integer"}
                    }
                }
            }
        }
    })
}
