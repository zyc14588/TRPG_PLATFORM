use std::collections::HashMap;

use api_server::ApiApplication;
use trpg_contracts::HttpRequest;
use trpg_identity::IdentityService;

const KEY: [u8; 32] = [0x36; 32];

fn request(method: &str, path: &str) -> HttpRequest {
    HttpRequest {
        method: method.to_owned(),
        path: path.to_owned(),
        headers: HashMap::new(),
        body: Vec::new(),
    }
}

#[test]
fn v1_schema_is_published_and_lifecycle_routes_authenticate_before_dispatch() {
    let application =
        ApiApplication::new(IdentityService::new(&KEY, 60_000).expect("identity service"));

    let schema = application
        .handle(&request("GET", "/api/v1/openapi.json"))
        .expect("OpenAPI route must be published");
    assert_eq!(schema.status, 200);
    assert_eq!(schema.body["openapi"], "3.1.0");

    let campaign_create = application
        .handle(&request("POST", "/api/v1/campaigns"))
        .expect("Campaign create route must be published");
    assert_eq!(campaign_create.status, 401);
    assert_eq!(campaign_create.body["error"], "AUTHENTICATION_REQUIRED");

    let gameplay_action = application
        .handle(&request(
            "POST",
            "/api/v1/campaigns/campaign_route_contract/gameplay-actions",
        ))
        .expect("public gameplay action route must be published");
    assert_eq!(gameplay_action.status, 401);
    assert_eq!(gameplay_action.body["error"], "AUTHENTICATION_REQUIRED");
}
