use std::sync::{Arc, Mutex};

use trpg_identity::{
    AuthenticationContext, CampaignMembership, CampaignRole, HttpAuthStatus, IdentityError,
    IdentityService,
};
use trpg_shared_kernel::EntityId;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApiAuthError {
    pub status: u16,
    pub code: &'static str,
}

impl From<IdentityError> for ApiAuthError {
    fn from(error: IdentityError) -> Self {
        let status = match error.http_status() {
            HttpAuthStatus::Unauthorized401 => 401,
            HttpAuthStatus::Forbidden403 => 403,
            HttpAuthStatus::Conflict409 => 409,
            HttpAuthStatus::Internal500 => 500,
        };
        Self {
            status,
            code: error.code(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthenticatedCampaignRequest {
    pub authentication: AuthenticationContext,
    pub membership: CampaignMembership,
}

#[derive(Clone)]
pub struct AuthenticationMiddleware {
    identity: Arc<Mutex<IdentityService>>,
}

impl AuthenticationMiddleware {
    pub fn new(identity: Arc<Mutex<IdentityService>>) -> Self {
        Self { identity }
    }

    pub fn authenticate_bearer(
        &self,
        authorization: Option<&str>,
        now_unix_ms: u64,
    ) -> Result<AuthenticationContext, ApiAuthError> {
        let token = authorization
            .and_then(|value| value.strip_prefix("Bearer "))
            .filter(|value| !value.is_empty())
            .ok_or(IdentityError::AuthenticationRequired)?;
        self.identity
            .lock()
            .map_err(|_| IdentityError::InvalidIdentityData)?
            .authenticate_session(Some(token), now_unix_ms)
            .map_err(ApiAuthError::from)
    }

    pub fn authorize_campaign(
        &self,
        authorization: Option<&str>,
        campaign_id: &str,
        allowed_roles: &[CampaignRole],
        now_unix_ms: u64,
    ) -> Result<AuthenticatedCampaignRequest, ApiAuthError> {
        let authentication = self.authenticate_bearer(authorization, now_unix_ms)?;
        let campaign_id = EntityId::new(campaign_id)
            .map_err(|_| ApiAuthError::from(IdentityError::InvalidIdentityData))?;
        let membership = self
            .identity
            .lock()
            .map_err(|_| IdentityError::InvalidIdentityData)?
            .require_membership(&authentication, &campaign_id, allowed_roles)?;
        Ok(AuthenticatedCampaignRequest {
            authentication,
            membership,
        })
    }

    pub fn identity(&self) -> &Arc<Mutex<IdentityService>> {
        &self.identity
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use trpg_identity::GlobalRole;

    const KEY: [u8; 32] = [9; 32];

    fn middleware() -> (AuthenticationMiddleware, String) {
        let mut identity = IdentityService::new(&KEY, 60_000).unwrap();
        identity
            .create_user(
                "user_owner",
                "owner@example.test",
                "correct horse battery staple",
                GlobalRole::ServerOwner,
            )
            .unwrap();
        identity
            .create_user(
                "user_player",
                "player@example.test",
                "another correct horse battery",
                GlobalRole::User,
            )
            .unwrap();
        let owner_session = identity
            .login("owner@example.test", "correct horse battery staple", 1_000)
            .unwrap();
        let owner = identity
            .authenticate_session(Some(owner_session.token.expose()), 1_001)
            .unwrap();
        identity
            .grant_membership(&owner, "campaign_a", "user_player", CampaignRole::Player)
            .unwrap();
        let player_session = identity
            .login(
                "player@example.test",
                "another correct horse battery",
                1_000,
            )
            .unwrap();
        (
            AuthenticationMiddleware::new(Arc::new(Mutex::new(identity))),
            player_session.token.expose().to_owned(),
        )
    }

    #[test]
    fn missing_session_is_401_and_cross_campaign_is_403() {
        let (middleware, token) = middleware();
        assert_eq!(
            middleware.authenticate_bearer(None, 1_002),
            Err(ApiAuthError {
                status: 401,
                code: "AUTHENTICATION_REQUIRED",
            })
        );
        let authorization = format!("Bearer {token}");
        assert_eq!(
            middleware.authorize_campaign(
                Some(&authorization),
                "campaign_b",
                &[CampaignRole::Player],
                1_002,
            ),
            Err(ApiAuthError {
                status: 403,
                code: "CAMPAIGN_MEMBERSHIP_REQUIRED",
            })
        );
        assert!(middleware
            .authorize_campaign(
                Some(&authorization),
                "campaign_a",
                &[CampaignRole::Player],
                1_002,
            )
            .is_ok());
    }
}
