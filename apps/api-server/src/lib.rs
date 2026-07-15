pub mod middleware;

use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use serde_json::json;
use trpg_contracts::{HttpRequest, HttpResponse};
use trpg_identity::{CampaignRole, IdentityError, IdentityService};
use trpg_security_governance::authorize_campaign_membership_change;
use trpg_security_governance::policy_adapter::OpenFgaOpaPolicyAdapter;
use trpg_security_governance::tamper_evident_audit::FileAuditLog;
use trpg_shared_kernel::{AuthorityMode, EntityId};

use middleware::{ApiAuthError, AuthenticationMiddleware};

#[derive(Clone)]
pub struct ApiApplication {
    authentication: AuthenticationMiddleware,
    identity_verifier: trpg_identity::IdentityVerifier,
    membership_governance: Option<Arc<Mutex<MembershipGovernance>>>,
}

struct MembershipGovernance {
    policy: OpenFgaOpaPolicyAdapter,
    audit: FileAuditLog,
}

impl ApiApplication {
    pub fn new(identity: IdentityService) -> Self {
        let identity_verifier = identity.verifier();
        Self {
            authentication: AuthenticationMiddleware::new(Arc::new(Mutex::new(identity))),
            identity_verifier,
            membership_governance: None,
        }
    }

    pub fn new_governed(
        identity: IdentityService,
        policy: OpenFgaOpaPolicyAdapter,
        audit: FileAuditLog,
    ) -> Self {
        let identity_verifier = identity.verifier();
        Self {
            authentication: AuthenticationMiddleware::new(Arc::new(Mutex::new(identity))),
            identity_verifier,
            membership_governance: Some(Arc::new(Mutex::new(MembershipGovernance {
                policy,
                audit,
            }))),
        }
    }

    pub fn readiness(&self) -> Result<String, String> {
        self.authentication
            .identity()
            .lock()
            .map_err(|_| "identity state lock poisoned".to_owned())?
            .check_readiness()
            .map(|()| "persistent identity and authorization state ready".to_owned())
            .map_err(|error| error.code().to_owned())
    }

    pub fn handle(&self, request: &HttpRequest) -> Option<HttpResponse> {
        match (request.method.as_str(), request.path.as_str()) {
            ("POST", "/auth/login") => Some(self.login(request)),
            ("POST", "/auth/refresh") => Some(self.refresh(request)),
            ("POST", "/auth/logout") => Some(self.logout(request)),
            _ => self.handle_campaign_route(request),
        }
    }

    fn login(&self, request: &HttpRequest) -> HttpResponse {
        let body: LoginRequest = match parse_json(request) {
            Ok(body) => body,
            Err(response) => return response,
        };
        let now = match now_unix_ms() {
            Ok(now) => now,
            Err(response) => return response,
        };
        match self.authentication.identity().lock() {
            Ok(mut identity) => match identity.login(&body.login, &body.password, now) {
                Ok(session) => HttpResponse::json(
                    200,
                    json!({
                        "access_token": session.token.expose(),
                        "token_type": "Bearer",
                        "expires_at_unix_ms": session.expires_at_unix_ms,
                    }),
                ),
                Err(error) => identity_error(error),
            },
            Err(_) => internal_error(),
        }
    }

    fn refresh(&self, request: &HttpRequest) -> HttpResponse {
        let token = match bearer_token(request) {
            Ok(token) => token,
            Err(response) => return response,
        };
        let now = match now_unix_ms() {
            Ok(now) => now,
            Err(response) => return response,
        };
        match self.authentication.identity().lock() {
            Ok(mut identity) => match identity.refresh_session(token, now) {
                Ok(session) => HttpResponse::json(
                    200,
                    json!({
                        "access_token": session.token.expose(),
                        "token_type": "Bearer",
                        "expires_at_unix_ms": session.expires_at_unix_ms,
                    }),
                ),
                Err(error) => identity_error(error),
            },
            Err(_) => internal_error(),
        }
    }

    fn logout(&self, request: &HttpRequest) -> HttpResponse {
        let token = match bearer_token(request) {
            Ok(token) => token,
            Err(response) => return response,
        };
        match self.authentication.identity().lock() {
            Ok(mut identity) => match identity.logout(token) {
                Ok(()) => HttpResponse::json(204, json!({})),
                Err(error) => identity_error(error),
            },
            Err(_) => internal_error(),
        }
    }

    fn handle_campaign_route(&self, request: &HttpRequest) -> Option<HttpResponse> {
        let segments = request
            .path
            .trim_matches('/')
            .split('/')
            .collect::<Vec<_>>();
        match (request.method.as_str(), segments.as_slice()) {
            ("GET", ["campaigns", campaign_id, "authority"]) => {
                Some(self.get_authority(request, campaign_id))
            }
            ("PUT", ["campaigns", campaign_id, "memberships", user_id]) => {
                Some(self.put_membership(request, campaign_id, user_id))
            }
            _ => None,
        }
    }

    fn get_authority(&self, request: &HttpRequest, campaign_id: &str) -> HttpResponse {
        if let Err(error) = self.authentication.authorize_campaign(
            request.header("authorization"),
            campaign_id,
            &[
                CampaignRole::CampaignOwner,
                CampaignRole::HumanKeeper,
                CampaignRole::Player,
                CampaignRole::Spectator,
            ],
            match now_unix_ms() {
                Ok(now) => now,
                Err(response) => return response,
            },
        ) {
            return auth_error(error);
        }
        let campaign_id = match EntityId::new(campaign_id) {
            Ok(campaign_id) => campaign_id,
            Err(_) => return HttpResponse::json(400, json!({"error": "INVALID_ENTITY_ID"})),
        };
        let contract = match self.authentication.identity().lock() {
            Ok(mut identity) => match identity.authority_contract(&campaign_id) {
                Ok(contract) => contract,
                Err(error) => return identity_error(error),
            },
            Err(_) => return internal_error(),
        };
        let Some(contract) = contract else {
            return HttpResponse::json(404, json!({"error": "AUTHORITY_CONTRACT_NOT_FOUND"}));
        };
        HttpResponse::json(
            200,
            json!({
                "contract_id": contract.contract_id().as_str(),
                "campaign_id": contract.campaign_id().as_str(),
                "mode": match contract.mode() {
                    AuthorityMode::HumanKp => "HUMAN_KP",
                    AuthorityMode::AiKp => "AI_KP",
                },
                "authority_owner": contract.authority_owner().as_str(),
                "version": contract.version(),
                "locked": contract.is_locked(),
                "change_policy": "FORK_ONLY",
            }),
        )
    }

    fn put_membership(
        &self,
        request: &HttpRequest,
        campaign_id: &str,
        user_id: &str,
    ) -> HttpResponse {
        let now = match now_unix_ms() {
            Ok(now) => now,
            Err(response) => return response,
        };
        let authentication = match self
            .authentication
            .authenticate_bearer(request.header("authorization"), now)
        {
            Ok(authentication) => authentication,
            Err(error) => return auth_error(error),
        };
        let body: MembershipRequest = match parse_json(request) {
            Ok(body) => body,
            Err(response) => return response,
        };
        let role = match parse_campaign_role(&body.role) {
            Ok(role) => role,
            Err(response) => return response,
        };
        let campaign_entity_id = match EntityId::new(campaign_id) {
            Ok(campaign_id) => campaign_id,
            Err(_) => return HttpResponse::json(400, json!({"error": "INVALID_ENTITY_ID"})),
        };
        let (acting_membership, authority) = match self.authentication.identity().lock() {
            Ok(mut identity) => {
                let membership = match identity.require_membership_manager(
                    &authentication,
                    &campaign_entity_id,
                    now,
                ) {
                    Ok(membership) => membership,
                    Err(error) => return identity_error(error),
                };
                let authority = match identity.authority_contract(&campaign_entity_id) {
                    Ok(Some(authority)) => authority,
                    Ok(None) => {
                        return HttpResponse::json(
                            404,
                            json!({"error": "AUTHORITY_CONTRACT_NOT_FOUND"}),
                        )
                    }
                    Err(error) => return identity_error(error),
                };
                (membership, authority)
            }
            Err(_) => return internal_error(),
        };
        let governance = match &self.membership_governance {
            Some(governance) => governance,
            None => {
                return HttpResponse::json(503, json!({"error": "POLICY_UNAVAILABLE"}));
            }
        };
        let mut governance = match governance.lock() {
            Ok(governance) => governance,
            Err(_) => return internal_error(),
        };
        let trace_id = format!(
            "membership_{}_{}",
            authentication.subject_id().as_str(),
            now
        );
        let policy = governance.policy.clone();
        if let Err(error) = authorize_campaign_membership_change(
            &policy,
            &mut governance.audit,
            &self.identity_verifier,
            &authentication,
            acting_membership.as_ref(),
            authority.mode(),
            &campaign_entity_id,
            user_id,
            role,
            &trace_id,
            now,
        ) {
            return HttpResponse::json(error.http_status(), json!({"error": error.code()}));
        }
        match self.authentication.identity().lock() {
            Ok(mut identity) => {
                match identity.grant_membership(&authentication, campaign_id, user_id, role, now) {
                    Ok(membership) => HttpResponse::json(
                        200,
                        json!({
                            "campaign_id": membership.campaign_id().as_str(),
                            "user_id": membership.user_id().as_str(),
                            "role": campaign_role_name(membership.role()),
                        }),
                    ),
                    Err(error) => identity_error(error),
                }
            }
            Err(_) => internal_error(),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LoginRequest {
    login: String,
    password: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MembershipRequest {
    role: String,
}

fn parse_json<T: for<'de> Deserialize<'de>>(request: &HttpRequest) -> Result<T, HttpResponse> {
    if request.header("content-type") != Some("application/json") {
        return Err(HttpResponse::json(
            400,
            json!({"error": "JSON_CONTENT_TYPE_REQUIRED"}),
        ));
    }
    serde_json::from_slice(&request.body)
        .map_err(|_| HttpResponse::json(400, json!({"error": "INVALID_JSON_BODY"})))
}

fn bearer_token(request: &HttpRequest) -> Result<&str, HttpResponse> {
    request
        .header("authorization")
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|token| !token.is_empty())
        .ok_or_else(|| HttpResponse::json(401, json!({"error": "AUTHENTICATION_REQUIRED"})))
}

fn parse_campaign_role(value: &str) -> Result<CampaignRole, HttpResponse> {
    match value {
        "CAMPAIGN_OWNER" => Ok(CampaignRole::CampaignOwner),
        "HUMAN_KEEPER" => Ok(CampaignRole::HumanKeeper),
        "PLAYER" => Ok(CampaignRole::Player),
        "SPECTATOR" => Ok(CampaignRole::Spectator),
        _ => Err(HttpResponse::json(
            400,
            json!({"error": "INVALID_CAMPAIGN_ROLE"}),
        )),
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

fn identity_error(error: IdentityError) -> HttpResponse {
    auth_error(ApiAuthError::from(error))
}

fn auth_error(error: ApiAuthError) -> HttpResponse {
    HttpResponse::json(error.status, json!({"error": error.code}))
}

fn internal_error() -> HttpResponse {
    HttpResponse::json(500, json!({"error": "IDENTITY_DATA_INVALID"}))
}

fn now_unix_ms() -> Result<u64, HttpResponse> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| internal_error())?
        .as_millis();
    u64::try_from(millis).map_err(|_| internal_error())
}
